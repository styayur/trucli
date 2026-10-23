use anyhow::{Context, Result, ensure};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::{net::IpAddr, path::Path};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub display_name: String,
    pub mode: String,
    #[serde(default)]
    pub process_hints: Vec<String>,
    #[serde(default)]
    pub interface_hints: Vec<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub domain_suffixes: Vec<String>,
    #[serde(default)]
    pub cidrs: Vec<IpNet>,
    #[serde(default)]
    pub dns_expectations: Vec<IpAddr>,
    #[serde(default)]
    pub notes: Vec<String>,
    pub require_campus_rules: bool,
}

pub const EXAMPLE: &str = r#"# Replace example.invalid using authorized institutional documentation.
# Empty CIDRs are deliberate: there is no universal campus address range.
id: example
display_name: Example campus (edit before use)
mode: system-proxy
process_hints: [aTrust, aTrustTray]
interface_hints: [aTrust, Sangfor]
domains: [portal.example.invalid]
domain_suffixes: [campus.example.invalid]
cidrs: []
dns_expectations: []
notes: [Obtain approved domains, CIDRs and DNS requirements from your institution.]
require_campus_rules: true
"#;

pub fn domain_valid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.is_ascii()
        && !s.ends_with('.')
        && s.contains('.')
        && s.parse::<IpAddr>().is_err()
        && s.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

impl Profile {
    pub fn load(path: &Path) -> Result<Self> {
        let text = crate::storage::read_text(path)?;
        let p: Self = crate::storage::yaml(&text).map_err(|_| {
            anyhow::anyhow!("Invalid profile YAML or fields; check schema, CIDRs and IP addresses")
        })?;
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty()
                && self.id.len() <= 48
                && self
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
            "Profile id must be 1-48 lowercase letters, digits or hyphens"
        );
        ensure!(
            !self.display_name.trim().is_empty()
                && self.display_name.len() <= 160
                && !self.display_name.chars().any(char::is_control),
            "Invalid display_name"
        );
        ensure!(
            self.mode == "system-proxy",
            "Only system-proxy mode is supported; TUN is unavailable"
        );
        for d in self.domains.iter().chain(&self.domain_suffixes) {
            ensure!(
                domain_valid(d),
                "Malformed domain; use ASCII DNS names without wildcards, ports or URLs"
            );
            ensure!(
                !d.ends_with(".invalid"),
                "Replace example.invalid placeholders using approved institutional documentation"
            );
        }
        ensure!(
            !self.require_campus_rules || !self.rules().is_empty(),
            "Profile requires at least one explicit campus rule"
        );
        for n in &self.cidrs {
            ensure!(
                n.prefix_len() > 0 && *n == n.trunc(),
                "CIDRs must be canonical network prefixes; default routes are not campus rules"
            );
        }
        for hint in self.process_hints.iter().chain(&self.interface_hints) {
            ensure!(
                !hint.trim().is_empty() && hint.len() <= 100 && !hint.chars().any(char::is_control),
                "Detection hints must be nonempty plain text"
            );
        }
        Ok(())
    }

    pub fn rules(&self) -> Vec<String> {
        self.domains
            .iter()
            .map(|d| format!("DOMAIN,{d},DIRECT"))
            .chain(
                self.domain_suffixes
                    .iter()
                    .map(|d| format!("DOMAIN-SUFFIX,{d},DIRECT")),
            )
            .chain(self.cidrs.iter().map(|n| {
                format!(
                    "{},{n},DIRECT,no-resolve",
                    if n.addr().is_ipv4() {
                        "IP-CIDR"
                    } else {
                        "IP-CIDR6"
                    }
                )
            }))
            .collect()
    }
}

pub fn resolve(home: &Path, name: &str) -> Result<std::path::PathBuf> {
    let path = Path::new(name);
    if path.is_file() {
        return path.canonicalize().context("Cannot resolve profile file");
    }
    ensure!(
        !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Unknown profile; specify a file or profile identifier"
    );
    Ok(home.join("profiles").join(format!("{name}.yaml")))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn profile() -> Profile {
        serde_saphyr::from_str(&EXAMPLE.replace("example.invalid", "example.edu")).unwrap()
    }
    #[test]
    fn validates_domains_without_rule_injection() {
        for s in [
            "https://a.edu",
            "*.a.edu",
            "a.edu,DIRECT",
            "a.edu\nMATCH,DIRECT",
            "a..edu",
            "-a.edu",
            "a-.edu",
            "a.edu:443",
            "a",
            "192.0.2.1",
            "a.edu.",
        ] {
            assert!(!domain_valid(s), "{s}");
        }
        assert!(domain_valid("library.example.edu"));
        assert!(domain_valid("xn--fiqs8s.example"));
    }
    #[test]
    fn placeholders_and_unsupported_modes_fail() {
        let p: Profile = serde_saphyr::from_str(EXAMPLE).unwrap();
        assert!(p.validate().is_err());
        let mut p = profile();
        p.mode = "tun".into();
        assert!(p.validate().is_err());
    }
    #[test]
    fn invalid_cidrs_and_unknown_fields_fail() {
        for cidr in ["10.0.0.0/999", "2001:db8::/129", "not-a-network"] {
            assert!(
                serde_saphyr::from_str::<Profile>(
                    &EXAMPLE.replace("cidrs: []", &format!("cidrs: ['{cidr}']"))
                )
                .is_err()
            );
        }
        assert!(
            serde_saphyr::from_str::<Profile>(&format!("{EXAMPLE}\npassword: secret")).is_err()
        );
        let mut p = profile();
        p.cidrs = vec!["192.0.2.1/24".parse().unwrap()];
        assert!(p.validate().is_err());
        p.cidrs = vec!["0.0.0.0/0".parse().unwrap()];
        assert!(p.validate().is_err());
    }
    #[test]
    fn valid_dual_stack_rules() {
        let mut p = profile();
        p.cidrs = vec![
            "192.0.2.0/24".parse().unwrap(),
            "2001:db8::/48".parse().unwrap(),
        ];
        p.validate().unwrap();
        assert!(
            p.rules()
                .contains(&"IP-CIDR6,2001:db8::/48,DIRECT,no-resolve".into())
        );
    }
}
