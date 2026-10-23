use crate::{config, mihomo, platform, profile::Profile, storage};
use anyhow::{Result, ensure};
use ipnet::IpNet;
use serde_json::{Value, json};
use std::path::Path;

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
fn hinted(s: &str, hints: &[String]) -> bool {
    hints
        .iter()
        .any(|h| s.to_ascii_lowercase().contains(&h.to_ascii_lowercase()))
}

// This allowlist intentionally excludes all arbitrary strings, addresses, paths and credentials.
pub fn summarize(raw: &Value, p: Option<&Profile>) -> Value {
    let default_process = vec!["atrust".into()];
    let default_interface = vec!["atrust".into(), "sangfor".into()];
    let process_hints = p.map(|p| &p.process_hints).unwrap_or(&default_process);
    let interface_hints = p.map(|p| &p.interface_hints).unwrap_or(&default_interface);
    let processes = array(raw, "processes")
        .iter()
        .filter(|v| hinted(v.as_str().unwrap_or(""), process_hints))
        .count();
    let interfaces: Vec<_> = array(raw, "adapters")
        .iter()
        .filter(|v| {
            hinted(v["Name"].as_str().unwrap_or(""), interface_hints)
                || hinted(
                    v["InterfaceDescription"].as_str().unwrap_or(""),
                    interface_hints,
                )
        })
        .collect();
    let up: Vec<_> = interfaces
        .iter()
        .filter(|i| i["Status"].as_str() == Some("Up"))
        .filter_map(|i| i["ifIndex"].as_u64())
        .collect();
    let campus = p.map(|p| p.cidrs.as_slice()).unwrap_or(&[]);
    let campus_routes = array(raw, "routes")
        .iter()
        .filter(|r| {
            let net = r["DestinationPrefix"]
                .as_str()
                .and_then(|s| s.parse::<IpNet>().ok());
            net.is_some_and(|net| {
                net.prefix_len() > 0
                    && campus
                        .iter()
                        .any(|c| net.contains(&c.addr()) && net.prefix_len() <= c.prefix_len())
                    && r["InterfaceIndex"]
                        .as_u64()
                        .is_some_and(|idx| up.contains(&idx))
            })
        })
        .count();
    let installed = !array(raw, "installed").is_empty();
    let assessment = if campus_routes > 0 && !up.is_empty() {
        "possible connection established (route/interface evidence only)"
    } else if !interfaces.is_empty() {
        "virtual interface hint detected; connection unknown"
    } else if processes > 0 {
        "process detected; authentication unknown"
    } else if installed {
        "installed component detected; connection unknown"
    } else {
        "not detected; hints may not match this installation"
    };
    let expected_dns_matches = array(raw, "dns")
        .iter()
        .flat_map(|d| array(d, "ServerAddresses"))
        .filter(|s| {
            s.as_str()
                .and_then(|s| s.parse().ok())
                .is_some_and(|ip| p.is_some_and(|p| p.dns_expectations.contains(&ip)))
        })
        .count();
    json!({
        "atrust_assessment": assessment, "authentication_verified": false, "resource_reachability_tested": false,
        "installed_component_detected": installed, "process_hint_matches": processes,
        "interface_hint_matches": interfaces.len(), "up_interface_hint_matches": up.len(),
        "campus_route_evidence_on_hint_interfaces": campus_routes,
        "configured_campus_cidrs": campus.len(), "dns_expectation_matches": expected_dns_matches,
        "adapter_count": array(raw, "adapters").len(), "route_count": array(raw, "routes").len(),
        "default_route_count": array(raw, "routes").iter().filter(|r| matches!(r["DestinationPrefix"].as_str(),Some("0.0.0.0/0" | "::/0"))).count(),
        "dns_assignment_count": array(raw, "dns").len(),
        "interface_metrics": array(raw, "interfaces").iter().map(|i| json!({"family": i["AddressFamily"].as_u64(), "metric": i["InterfaceMetric"].as_u64()})).collect::<Vec<_>>()
    })
}
pub fn run(
    home: &Path,
    p: Option<&Profile>,
    config_path: Option<&Path>,
    exe: Option<&Path>,
    export: Option<&Path>,
) -> Result<()> {
    let raw = platform::evidence();
    let success = raw.is_ok();
    let mut report = summarize(&raw.unwrap_or(Value::Null), p);
    report["schema"] = json!(1);
    report["version"] = json!(env!("CARGO_PKG_VERSION"));
    report["network_evidence_available"] = json!(success);
    if !success {
        report["atrust_assessment"] =
            json!("unknown / insufficient evidence; Windows PowerShell/CIM query failed");
    }
    report["mihomo_executable_available"] = json!(mihomo::discover(exe)?.is_some());
    report["runtime"] = crate::runtime::report(home);
    if let Ok(proxy) = platform::proxy_read() {
        report["system_proxy"] = json!({"enabled": proxy.flags & 2 != 0, "pac_enabled": proxy.flags & 4 != 0,
            "auto_detect_enabled": proxy.flags & 8 != 0, "server_present": proxy.server.is_some()});
    } else {
        report["system_proxy"] = json!({"available": false});
    }
    if let Some(path) = config_path {
        let map = config::parse(&storage::read_text(path)?)?;
        report["fake_ip_overlap"] = json!(config::fake_overlap(
            &map,
            p.map(|p| p.cidrs.as_slice()).unwrap_or(&[])
        )?);
        report["runtime_config_supported"] = json!(config::safe_runtime(&map).is_ok());
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    if let Some(path) = export {
        storage::create(path, &serde_json::to_vec_pretty(&report)?)?;
        println!("Redacted report created; review before sharing.");
    }
    ensure!(
        success,
        "Network evidence incomplete; check availability of Windows PowerShell and NetTCPIP/CIM cmdlets"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redaction_is_allowlisted() {
        let raw = json!({"processes":["aTrust","credential=SECRET"],"installed":["C:\\Users\\Person"],"adapters":[{"Name":"SECRET","InterfaceDescription":"subscription?token=SECRET"}],"dns":[{"ServerAddresses":["192.0.2.3"]}],"routes":[{"NextHop":"192.0.2.1","DestinationPrefix":"192.0.2.0/24"}],"token":"SECRET"});
        let s = summarize(&raw, None).to_string();
        for sensitive in ["SECRET", "192.0.2", "Person", "subscription", "credential"] {
            assert!(!s.contains(sensitive));
        }
        assert_eq!(summarize(&raw, None)["authentication_verified"], false);
    }
}
