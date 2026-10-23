#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyState {
    pub flags: u32,
    pub server: Option<String>,
    pub bypass: Option<String>,
    pub auto_config_url: Option<String>,
}
impl ProxyState {
    pub fn applied(port: u16, previous: &Self) -> Self {
        Self {
            flags: 3,
            server: Some(format!("127.0.0.1:{port}")),
            bypass: Some("<local>".into()),
            auto_config_url: previous.auto_config_url.clone(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Identity {
    pub pid: u32,
    pub created: u64,
    pub executable: std::path::PathBuf,
}

#[cfg(not(windows))]
mod unsupported {
    use super::*;
    pub fn listener_owned(_: u32, _: u16) -> Result<bool> {
        bail!("Windows required")
    }
    use anyhow::{Result, bail};
    pub fn proxy_read() -> Result<ProxyState> {
        bail!("Windows 10/11 x86-64 is required")
    }
    pub fn proxy_write(_: &ProxyState) -> Result<()> {
        bail!("Windows required")
    }
    pub fn evidence() -> Result<serde_json::Value> {
        bail!("Windows required")
    }
    pub fn identity(_: u32) -> Result<Option<Identity>> {
        bail!("Windows required")
    }
    pub fn terminate(_: &Identity) -> Result<bool> {
        bail!("Windows required")
    }
    pub struct Pending;
    impl Pending {
        pub fn launch(
            _: &std::path::Path,
            _: &std::path::Path,
            _: &std::path::Path,
        ) -> Result<Self> {
            bail!("Windows required")
        }
        pub fn identity(&self) -> Result<Identity> {
            bail!("Windows required")
        }
        pub fn resume(self) -> Result<()> {
            bail!("Windows required")
        }
    }
}
#[cfg(not(windows))]
pub use unsupported::*;
