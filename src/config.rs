use crate::{profile::Profile, storage};
use anyhow::{Context, Result, ensure};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use serde_json::Value;
type Mapping = serde_json::Map<String, Value>;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub digest: String,
    pub campus_rules: usize,
}
pub fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
pub fn parse(text: &str) -> Result<Mapping> {
    let v: Value = storage::yaml(text).map_err(|_| {
        anyhow::anyhow!("Malformed Mihomo YAML (details suppressed to protect secrets)")
    })?;
    let map = v
        .as_object()
        .context("Mihomo YAML must be a mapping")?
        .clone();
    if let Some(rules) = map.get("rules") {
        ensure!(
            rules
                .as_array()
                .is_some_and(|a| a.iter().all(|v| v.as_str().is_some())),
            "rules must be a list of strings"
        );
    }
    Ok(map)
}
pub fn overlaps(a: &IpNet, b: &IpNet) -> bool {
    a.contains(&b.addr()) || b.contains(&a.addr())
}
pub fn fake_overlap(map: &Mapping, cidrs: &[IpNet]) -> Result<bool> {
    let Some(dns) = map.get("dns") else {
        return Ok(false);
    };
    let dns = dns.as_object().context("dns must be a mapping")?;
    // Mihomo defaults to this pool when Fake-IP is active and no explicit range exists.
    if dns.get("enhanced-mode").and_then(Value::as_str) != Some("fake-ip") {
        return Ok(false);
    }
    for key in ["fake-ip-range", "fake-ip-range6"] {
        let fallback = if key == "fake-ip-range" {
            Some("198.18.0.1/16")
        } else {
            None
        };
        if let Some(s) = dns.get(key).and_then(Value::as_str).or(fallback) {
            let net: IpNet = s.parse().context("Invalid Fake-IP range")?;
            if cidrs.iter().any(|c| overlaps(c, &net)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn provider_paths(
    map: &mut Mapping,
    mut visit: impl FnMut(&mut Value) -> Result<()>,
) -> Result<()> {
    for key in ["proxy-providers", "rule-providers"] {
        if let Some(providers) = map.get_mut(key) {
            for provider in providers
                .as_object_mut()
                .context("providers must be mappings")?
                .values_mut()
            {
                let provider = provider
                    .as_object_mut()
                    .context("provider must be a mapping")?;
                if let Some(path) = provider.get_mut("path") {
                    visit(path)?;
                }
            }
        }
    }
    Ok(())
}
pub fn merge(mut map: Mapping, p: &Profile, base: &Path) -> Result<Mapping> {
    p.validate()?;
    ensure!(
        map.get("mode").is_none_or(|m| m.as_str() == Some("rule")),
        "Base mode must be rule; campus rules cannot work in global/direct mode"
    );
    ensure!(
        !fake_overlap(&map, &p.cidrs)?,
        "Campus CIDR overlaps the Fake-IP pool; adjust DNS configuration first"
    );
    let mut rules: Vec<Value> = p.rules().into_iter().map(Value::String).collect();
    if let Some(old) = map.remove("rules") {
        rules.extend(
            old.as_array()
                .context("rules must be a list")?
                .iter()
                .cloned(),
        );
    }
    map.insert("rules".into(), rules.into());
    // Preserve all base fields; reject unsafe listener settings at start instead of rewriting them.
    provider_paths(&mut map, |v| {
        let path = Path::new(v.as_str().context("Provider path must be a string")?);
        if path.is_relative() {
            *v = Value::String(base.join(path).to_string_lossy().into_owned());
        }
        Ok(())
    })?;
    Ok(map)
}
pub fn build(home: &Path, p: &Profile, input: &Path) -> Result<PathBuf> {
    let input = input
        .canonicalize()
        .context("Base configuration unavailable")?;
    let mut map = merge(
        parse(&storage::read_text(&input)?)?,
        p,
        input.parent().unwrap(),
    )?;
    // Snapshot provider inputs/caches so engine refreshes never write to the user's originals.
    provider_paths(&mut map, |v| {
        let source = Path::new(v.as_str().context("Provider path must be a string")?);
        let target = home
            .canonicalize()?
            .join("engine")
            .join(format!("provider-{}.dat", storage::nonce()));
        if source.exists() {
            ensure!(
                source.is_file() && std::fs::metadata(source)?.len() <= 32 * 1024 * 1024,
                "Provider input must be a file no larger than 32 MiB"
            );
            storage::create(&target, &std::fs::read(source)?)?;
        }
        *v = Value::String(target.to_string_lossy().into_owned());
        Ok(())
    })?;
    let text = serde_saphyr::to_string(&map)?;
    let path = home
        .join("generated")
        .join(format!("{}-{}.yaml", p.id, storage::nonce()));
    storage::create(&path, text.as_bytes())?;
    storage::save_json(
        &path.with_extension("json"),
        &Manifest {
            schema: 1,
            digest: digest(text.as_bytes()),
            campus_rules: p.rules().len(),
        },
    )?;
    println!(
        "Generated {}\nPrepended {} campus DIRECT rules; original configuration preserved. Provider paths use Trucli-owned snapshots/caches.",
        path.display(),
        p.rules().len()
    );
    Ok(path)
}
pub fn owned(home: &Path, config: &Path) -> Result<(PathBuf, Mapping)> {
    let path = config
        .canonicalize()
        .context("Generated configuration unavailable")?;
    ensure!(
        path.parent() == Some(home.join("generated").canonicalize()?.as_path()),
        "start accepts only Trucli generated configurations"
    );
    let m: Manifest = storage::read_json(&path.with_extension("json"))?;
    let text = storage::read_text(&path)?;
    ensure!(
        m.schema == 1 && m.digest == digest(text.as_bytes()),
        "Generated configuration changed; rebuild it from the original input"
    );
    Ok((path, parse(&text)?))
}
pub fn safe_runtime(map: &Mapping) -> Result<u16> {
    ensure!(
        map.get("mode").is_none_or(|x| x.as_str() == Some("rule")),
        "Runtime requires rule mode"
    );
    ensure!(
        map.get("allow-lan")
            .is_none_or(|x| x.as_bool() == Some(false)),
        "Set allow-lan: false in the base and rebuild"
    );
    ensure!(
        map.get("bind-address")
            .is_none_or(|x| x.as_str() == Some("127.0.0.1")),
        "Set bind-address: 127.0.0.1 and rebuild"
    );
    for key in [
        "external-controller",
        "external-controller-tls",
        "external-controller-unix",
        "external-controller-pipe",
        "external-doh-server",
        "external-ui",
        "external-ui-name",
        "external-ui-url",
        "ss-config",
        "vmess-config",
        "tuic-server",
        "listeners",
        "tunnels",
        "iptables",
        "script",
        "authentication",
        "interface-name",
        "ntp",
    ] {
        ensure!(
            !map.contains_key(key),
            "Unsupported runtime field {key}; remove it from the base and rebuild"
        );
    }
    for key in ["tun", "dns"] {
        if let Some(v) = map.get(key) {
            let m = v.as_object().context("tun/dns must be mappings")?;
            ensure!(
                m.get("enable").is_none_or(|v| v.as_bool() == Some(false)),
                "Runtime requires {key}.enable: false; TUN and local DNS servers are unsupported"
            );
        }
    }
    for key in ["redir-port", "tproxy-port"] {
        ensure!(
            map.get(key).is_none_or(|v| v.as_u64() == Some(0)),
            "Transparent proxy ports are unsupported"
        );
    }
    for key in ["socks-port", "port", "mixed-port"] {
        if let Some(v) = map.get(key) {
            ensure!(v.as_u64().is_some_and(|n| n <= 65535), "Invalid proxy port");
        }
    }
    let port = ["mixed-port", "port"]
        .iter()
        .find_map(|key| map.get(*key).and_then(Value::as_u64).filter(|p| *p > 0))
        .context("Set mixed-port or port to a nonzero TCP port in the base")?;
    Ok(port as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> Profile {
        serde_saphyr::from_str(&crate::profile::EXAMPLE.replace("example.invalid", "example.edu"))
            .unwrap()
    }
    #[test]
    fn campus_precedence_and_semantic_preservation() {
        let input = parse("mixed-port: 7890\nproxies: [{name: chosen, type: direct}]\nproxy-groups: [{name: Group, type: select, proxies: [chosen]}]\nrules: ['MATCH,Group']\nsecret: private\n").unwrap();
        let original = input.clone();
        let output = merge(input, &profile(), Path::new("C:/base")).unwrap();
        for key in ["mixed-port", "proxies", "proxy-groups", "secret"] {
            assert_eq!(output.get(key), original.get(key));
        }
        let rules = output["rules"].as_array().unwrap();
        assert_eq!(rules[0].as_str(), Some("DOMAIN,portal.example.edu,DIRECT"));
        assert_eq!(&rules[2..], original["rules"].as_array().unwrap());
    }
    #[test]
    fn malformed_yaml_and_rules_rejected() {
        for s in [
            "[a,b]",
            "rules: [",
            "rules: nope",
            "rules: [123]",
            "rules: []\nrules: []",
        ] {
            assert!(parse(s).is_err(), "{s}");
        }
    }
    #[test]
    fn overlap_handles_v4_v6_and_default_pool() {
        let v4 = parse("dns: {enhanced-mode: fake-ip}").unwrap();
        assert!(fake_overlap(&v4, &["198.18.2.0/24".parse().unwrap()]).unwrap());
        assert!(!fake_overlap(&v4, &["192.0.2.0/24".parse().unwrap()]).unwrap());
        let v6 = parse("dns: {enhanced-mode: fake-ip, fake-ip-range6: '2001:db8::/48'}").unwrap();
        assert!(fake_overlap(&v6, &["2001:db8:0:1::/64".parse().unwrap()]).unwrap());
        assert!(!fake_overlap(&v6, &["2001:db9::/32".parse().unwrap()]).unwrap());
    }
    #[test]
    fn rejects_global_mode_and_fake_overlap() {
        assert!(merge(parse("mode: global").unwrap(), &profile(), Path::new(".")).is_err());
        let mut p = profile();
        p.cidrs = vec!["198.18.0.0/16".parse().unwrap()];
        assert!(
            merge(
                parse("dns: {enhanced-mode: fake-ip}").unwrap(),
                &p,
                Path::new(".")
            )
            .is_err()
        );
    }
    #[test]
    fn startup_refuses_unsupported_listeners() {
        for extra in [
            "tun: {enable: true}",
            "dns: {enable: true}",
            "allow-lan: true",
            "bind-address: '*'",
            "external-controller: '0.0.0.0:9090'",
            "ss-config: 'ss://example'",
            "vmess-config: 'vmess://example'",
            "tuic-server: {enable: true, listen: '0.0.0.0:10443'}",
            "external-doh-server: /dns-query",
            "listeners: []",
            "authentication: [user:password]",
            "mixed-port: 999999",
        ] {
            assert!(
                safe_runtime(&parse(&format!("port: 7890\n{extra}")).unwrap()).is_err(),
                "{extra}"
            );
        }
        assert_eq!(
            safe_runtime(&parse("mixed-port: 7890").unwrap()).unwrap(),
            7890
        );
    }
    #[test]
    fn relative_provider_paths_are_resolved() {
        let m = merge(
            parse("proxy-providers: {x: {type: file, path: provider.yaml}}").unwrap(),
            &profile(),
            Path::new("C:/base"),
        )
        .unwrap();
        assert_eq!(
            m["proxy-providers"]["x"]["path"].as_str().unwrap(),
            Path::new("C:/base").join("provider.yaml").to_string_lossy()
        );
    }
    #[test]
    fn websocket_paths_are_never_treated_as_files() {
        let base =
            parse("proxies: [{name: sample, type: vmess, ws-opts: {path: /websocket}}]").unwrap();
        let merged = merge(base.clone(), &profile(), Path::new("C:/base")).unwrap();
        assert_eq!(merged["proxies"], base["proxies"]);
    }
}
