use crate::{
    config, mihomo,
    platform::{self, Identity, ProxyState},
    storage,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    net::{SocketAddr, TcpListener, TcpStream},
    path::Path,
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
struct State {
    schema: u32,
    previous: ProxyState,
    applied: ProxyState,
    child: Option<Identity>,
}

pub fn report(home: &Path) -> serde_json::Value {
    let path = home.join("runtime.json");
    if !path.is_file() {
        return serde_json::json!({"metadata_present": false});
    }
    match storage::read_json::<State>(&path) {
        Ok(s) => serde_json::json!({"metadata_present": true, "metadata_valid": s.schema == 1,
            "owned_child_running": s.child.as_ref().and_then(|c| platform::identity(c.pid).ok().flatten().map(|actual| actual == *c)),
            "proxy_still_matches_applied": platform::proxy_read().ok().map(|current| current == s.applied),
            "recovery_command": "trucli stop"}),
        Err(_) => serde_json::json!({"metadata_present": true, "metadata_valid": false}),
    }
}

// All mutation used during recovery passes through this small boundary for mock tests.
trait Recovery {
    fn read(&mut self) -> Result<ProxyState>;
    fn write(&mut self, value: &ProxyState) -> Result<()>;
    fn kill(&mut self, child: &Identity) -> Result<bool>;
}
struct Windows;
impl Recovery for Windows {
    fn read(&mut self) -> Result<ProxyState> {
        platform::proxy_read()
    }
    fn write(&mut self, value: &ProxyState) -> Result<()> {
        platform::proxy_write(value)
    }
    fn kill(&mut self, child: &Identity) -> Result<bool> {
        platform::terminate(child)
    }
}
fn recover(io: &mut impl Recovery, state: &State) -> Result<bool> {
    let current = io.read()?;
    let conflict = current != state.applied && current != state.previous;
    if current == state.applied {
        io.write(&state.previous)?;
        ensure!(
            io.read()? == state.previous,
            "Proxy restoration could not be verified; state retained"
        );
    }
    if let Some(child) = &state.child {
        io.kill(child)?;
    }
    Ok(conflict)
}
pub fn start(home: &Path, path: &Path, explicit: Option<&Path>) -> Result<()> {
    let home = home.canonicalize().context("Run trucli init first")?;
    let _lock = storage::lock(&home)?;
    let state_path = home.join("runtime.json");
    ensure!(
        !state_path.exists(),
        "Recovery state already exists; run trucli stop before starting again"
    );
    let (config, map) = config::owned(&home, path)?;
    let port = config::safe_runtime(&map)?;
    // Check every declared TCP listener before launch; an occupied port is never treated as our child.
    let mut guards = Vec::new();
    for key in ["mixed-port", "port", "socks-port"] {
        if let Some(p) = map
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .filter(|p| *p > 0)
        {
            guards.push(
                TcpListener::bind(("127.0.0.1", p as u16))
                    .context("A configured listener port is already occupied")?,
            );
        }
    }
    let exe = mihomo::validate(&home, &config, explicit, true)?.unwrap();
    let previous = platform::proxy_read()?;
    let mut state = State {
        schema: 1,
        applied: ProxyState::applied(port, &previous),
        previous,
        child: None,
    };
    storage::save_json(&state_path, &state)?;
    let outcome = (|| -> Result<()> {
        let pending = platform::Pending::launch(&exe, &config, &home.join("engine"))?;
        let child = pending.identity()?;
        state.child = Some(child.clone());
        storage::save_json(&state_path, &state)?;
        drop(guards);
        pending.resume()?;
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            ensure!(
                platform::identity(child.pid)?.as_ref() == Some(&child),
                "Mihomo exited during startup; system proxy was not applied"
            );
            if platform::listener_owned(child.pid, port)?
                && TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok()
            {
                break;
            }
            ensure!(
                Instant::now() < deadline,
                "Mihomo listener did not become ready within 10s"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        ensure!(
            platform::proxy_read()? == state.previous,
            "Proxy settings changed during startup; refusing to overwrite them"
        );
        platform::proxy_write(&state.applied)?;
        ensure!(
            platform::proxy_read()? == state.applied,
            "Applied proxy state could not be verified"
        );
        Ok(())
    })();
    if let Err(error) = outcome {
        if recover(&mut Windows, &state).is_ok() {
            std::fs::remove_file(&state_path)?;
        }
        return Err(error).context("Startup failed; run stop if recovery metadata remains");
    }
    println!(
        "Started owned Mihomo process. Current-user system proxy: 127.0.0.1:{port}. Run trucli stop to recover. Apps ignoring system proxy are unaffected; aTrust authentication is not verified."
    );
    Ok(())
}
pub fn stop(home: &Path) -> Result<()> {
    let _lock = storage::lock(home)?;
    let path = home.join("runtime.json");
    if !path.exists() {
        println!("No Trucli runtime state; nothing changed.");
        return Ok(());
    }
    let state: State = storage::read_json(&path)?;
    ensure!(
        state.schema == 1,
        "Unknown runtime schema; preserve metadata for recovery"
    );
    let conflict = recover(&mut Windows, &state)?;
    std::fs::remove_file(path)?;
    ensure!(
        !conflict,
        "Stopped owned child; newer system proxy settings were preserved without restoration"
    );
    println!(
        "Owned child stopped if still present; previous proxy state verified. No routes, DNS or WinHTTP settings were changed."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Mock {
        current: ProxyState,
        writes: usize,
        kills: usize,
        fail_write: bool,
    }
    impl Recovery for Mock {
        fn read(&mut self) -> Result<ProxyState> {
            Ok(self.current.clone())
        }
        fn write(&mut self, p: &ProxyState) -> Result<()> {
            ensure!(!self.fail_write, "mock failure");
            self.current = p.clone();
            self.writes += 1;
            Ok(())
        }
        fn kill(&mut self, _: &Identity) -> Result<bool> {
            self.kills += 1;
            Ok(true)
        }
    }
    fn state() -> State {
        let previous = ProxyState {
            flags: 13,
            server: None,
            bypass: None,
            auto_config_url: Some("https://private.invalid/pac?token=secret".into()),
        };
        State {
            schema: 1,
            applied: ProxyState::applied(7890, &previous),
            previous,
            child: Some(Identity {
                pid: 123,
                created: 999,
                executable: "mihomo.exe".into(),
            }),
        }
    }
    #[test]
    fn restores_exact_owned_state() {
        let s = state();
        let mut m = Mock {
            current: s.applied.clone(),
            writes: 0,
            kills: 0,
            fail_write: false,
        };
        assert!(!recover(&mut m, &s).unwrap());
        assert_eq!(m.current, s.previous);
        assert_eq!((m.writes, m.kills), (1, 1));
    }
    #[test]
    fn preserves_newer_settings() {
        let s = state();
        let mut p = s.applied.clone();
        p.flags |= 8;
        let mut m = Mock {
            current: p.clone(),
            writes: 0,
            kills: 0,
            fail_write: false,
        };
        assert!(recover(&mut m, &s).unwrap());
        assert_eq!(m.current, p);
        assert_eq!(m.writes, 0);
    }
    #[test]
    fn already_restored_is_idempotent() {
        let s = state();
        let mut m = Mock {
            current: s.previous.clone(),
            writes: 0,
            kills: 0,
            fail_write: false,
        };
        assert!(!recover(&mut m, &s).unwrap());
        assert_eq!(m.writes, 0);
    }
    #[test]
    fn failed_restore_does_not_kill_listener() {
        let s = state();
        let mut m = Mock {
            current: s.applied.clone(),
            writes: 0,
            kills: 0,
            fail_write: true,
        };
        assert!(recover(&mut m, &s).is_err());
        assert_eq!(m.kills, 0);
    }
    #[test]
    fn interrupted_before_child_is_recoverable() {
        let mut s = state();
        s.child = None;
        let mut m = Mock {
            current: s.previous.clone(),
            writes: 0,
            kills: 0,
            fail_write: false,
        };
        assert!(!recover(&mut m, &s).unwrap());
        assert_eq!((m.writes, m.kills), (0, 0));
    }
}
