use crate::{config, storage};
use anyhow::{Context, Result, ensure};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn discover(explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(p) = explicit {
        let p = p
            .canonicalize()
            .context("Explicit Mihomo executable unavailable")?;
        ensure!(
            p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")),
            "Mihomo must be an existing .exe file"
        );
        return Ok(Some(p));
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        if !dir.is_absolute() {
            continue;
        }
        let p = dir.join("mihomo.exe");
        if p.is_file() {
            return Ok(Some(p.canonicalize()?));
        }
    }
    Ok(None)
}
pub fn command(exe: &Path) -> Command {
    let mut cmd = Command::new(exe);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for (key, _) in std::env::vars_os() {
        let k = key.to_string_lossy().to_ascii_uppercase();
        if k.starts_with("CLASH_") || k.starts_with("MIHOMO_") {
            cmd.env_remove(key);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
pub fn validate(
    home: &Path,
    path: &Path,
    explicit: Option<&Path>,
    required: bool,
) -> Result<Option<PathBuf>> {
    config::parse(&storage::read_text(path)?)?;
    println!("Structural YAML validation passed (not full Mihomo validation).");
    let Some(exe) = discover(explicit)? else {
        ensure!(
            !required,
            "Mihomo unavailable; specify --mihomo with your trusted official installation"
        );
        println!("Mihomo validation NOT performed: executable unavailable.");
        return Ok(None);
    };
    config::owned(home, path).context("Native validation requires an unchanged Trucli-generated config; run config build first to isolate provider caches")?;
    std::fs::create_dir_all(home.join("engine"))?;
    let mut child = command(&exe)
        .arg("-t")
        .arg("-d")
        .arg(home.join("engine"))
        .arg("-f")
        .arg(path.canonicalize()?)
        .spawn()
        .context("Cannot launch trusted Mihomo validator")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait()? {
            ensure!(
                status.success(),
                "Mihomo validation failed; run mihomo -t -f locally for private details"
            );
            break;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("Mihomo validation timed out after 30s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("Mihomo executable validation passed.");
    Ok(Some(exe))
}
