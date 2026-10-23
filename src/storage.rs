use anyhow::{Context, Result, ensure};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn home(override_path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        return Ok(p);
    }
    Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is unavailable; specify --home")?,
    )
    .join("Trucli"))
}
pub fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
pub fn read_text(path: &Path) -> Result<String> {
    ensure!(
        fs::metadata(path)
            .context("File unavailable; check the supplied path")?
            .len()
            <= 8 * 1024 * 1024,
        "File exceeds the 8 MiB limit"
    );
    fs::read_to_string(path).context("Cannot read UTF-8 file")
}

pub fn yaml<T: serde::de::DeserializeOwned>(text: &str) -> Result<T> {
    let mut options = serde_saphyr::Options::default();
    options.reject_unsupported_tags = true;
    serde_saphyr::from_str_with_options(text, options).map_err(|_| anyhow::anyhow!("Invalid YAML, unsupported tags or resource budget exceeded; check structure (values suppressed)"))
}
pub fn create(path: &Path, data: &[u8]) -> Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .context("Cannot create file; existing files are never replaced")?;
    f.write_all(data)?;
    f.sync_all()?;
    Ok(())
}
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_str(&read_text(path)?)
        .context("Invalid Trucli metadata; preserve it for local recovery")
}
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", nonce()));
    create(&tmp, &serde_json::to_vec_pretty(value)?)?;
    fs::rename(&tmp, path).context("Cannot persist recovery metadata")
}
pub fn lock(home: &Path) -> Result<File> {
    ensure!(home.is_dir(), "Run trucli init first");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(home.join("operation.lock"))?;
    file.try_lock()
        .context("Another Trucli mutation is in progress")?;
    Ok(file)
}
pub fn init(home: &Path) -> Result<()> {
    fs::create_dir_all(home.join("profiles"))?;
    fs::create_dir_all(home.join("generated"))?;
    fs::create_dir_all(home.join("engine"))?;
    let example = home.join("profiles/example.yaml");
    if !example.exists() {
        create(&example, crate::profile::EXAMPLE.as_bytes())?;
    }
    println!(
        "Initialized {}\nEdit profiles/example.yaml using institutional documentation. Supply your existing Mihomo YAML and a trusted mihomo.exe with --mihomo. Example placeholders intentionally fail validation.",
        home.display()
    );
    Ok(())
}
