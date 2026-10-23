use std::process::Command;
fn cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_trucli"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn every_help_surface_works() {
    for args in [
        vec!["--help"],
        vec!["init", "--help"],
        vec!["status", "--help"],
        vec!["doctor", "--help"],
        vec!["profile", "--help"],
        vec!["profile", "list", "--help"],
        vec!["profile", "validate", "--help"],
        vec!["config", "--help"],
        vec!["config", "build", "--help"],
        vec!["config", "validate", "--help"],
        vec!["start", "--help"],
        vec!["stop", "--help"],
    ] {
        let output = cli(&args);
        assert!(output.status.success(), "{args:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    }
}
#[test]
fn version_agrees() {
    assert!(
        String::from_utf8_lossy(&cli(&["--version"]).stdout).contains(env!("CARGO_PKG_VERSION"))
    );
}
#[test]
fn bad_commands_return_syntax_errors() {
    for args in [
        vec!["start"],
        vec!["start", "--mode", "tun", "--config", "x"],
        vec!["config", "build"],
        vec!["unknown"],
    ] {
        assert_eq!(cli(&args).status.code(), Some(2));
    }
}
#[test]
fn init_is_idempotent_and_placeholder_rejected() {
    let home = std::env::temp_dir().join(format!(
        "trucli-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let h = home.to_str().unwrap();
    assert!(cli(&["--home", h, "init"]).status.success());
    let original = std::fs::read(home.join("profiles/example.yaml")).unwrap();
    assert!(cli(&["--home", h, "init"]).status.success());
    assert_eq!(
        original,
        std::fs::read(home.join("profiles/example.yaml")).unwrap()
    );
    assert_eq!(
        cli(&["--home", h, "profile", "validate", "example"])
            .status
            .code(),
        Some(1)
    );
    // Only this test's uniquely created directory is removed; real user state is untouched.
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn build_keeps_source_and_checks_generated_ownership() {
    let home = std::env::temp_dir().join(format!(
        "trucli-build-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let h = home.to_str().unwrap();
    assert!(cli(&["--home", h, "init"]).status.success());
    let profile = home.join("profiles/example.yaml");
    let text = std::fs::read_to_string(&profile)
        .unwrap()
        .replace("example.invalid", "example.edu");
    std::fs::write(&profile, text).unwrap();
    let source = home.join("base.yaml");
    let base = "mixed-port: 17890\nrule-providers: {sample: {type: file, behavior: classical, path: rules.yaml}}\nrules: ['MATCH,DIRECT']\n";
    let provider_original = home.join("rules.yaml");
    std::fs::write(&provider_original, "payload: ['DOMAIN,example.edu']\n").unwrap();
    std::fs::write(&source, base).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_trucli"))
        .args([
            "--home",
            h,
            "config",
            "build",
            "--profile",
            "example",
            "--input",
            source.to_str().unwrap(),
        ])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Mihomo validation NOT performed"));
    assert_eq!(std::fs::read_to_string(&source).unwrap(), base);
    let generated = std::fs::read_dir(home.join("generated"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "yaml"))
        .unwrap();
    let text = std::fs::read_to_string(&generated).unwrap();
    let parsed: serde_json::Value = serde_saphyr::from_str(&text).unwrap();
    let snapshot =
        std::path::Path::new(parsed["rule-providers"]["sample"]["path"].as_str().unwrap());
    assert!(snapshot.starts_with(home.canonicalize().unwrap().join("engine")));
    assert_ne!(snapshot, provider_original);
    assert_eq!(
        std::fs::read(snapshot).unwrap(),
        std::fs::read(&provider_original).unwrap()
    );
    std::fs::write(snapshot, "payload: []").unwrap();
    assert_eq!(
        std::fs::read_to_string(&provider_original).unwrap(),
        "payload: ['DOMAIN,example.edu']\n"
    );
    assert!(
        text.find("DOMAIN,portal.example.edu,DIRECT").unwrap() < text.find("MATCH,DIRECT").unwrap()
    );
    std::fs::write(&generated, format!("{text}\n# changed\n")).unwrap();
    let output = cli(&[
        "--home",
        h,
        "start",
        "--mode",
        "system-proxy",
        "--config",
        generated.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("changed"));
    assert!(!home.join("runtime.json").exists());
    std::fs::remove_dir_all(home).unwrap();
}
