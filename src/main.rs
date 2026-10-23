mod config;
mod diagnostics;
mod mihomo;
mod platform;
mod profile;
mod runtime;
mod storage;

use anyhow::{Result, ensure};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "Experimental Windows aTrust/Mihomo coordinator",
    after_help = "Copyright (C) 2026 styayur. AGPL-3.0-only; no warranty. See LICENSE. Exit codes: 0 success, 1 operational/validation error, 2 command syntax error."
)]
struct Cli {
    /// User-owned state directory (default: LOCALAPPDATA/Trucli).
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    /// Trusted Mihomo executable; otherwise discover mihomo.exe on PATH.
    #[arg(long, global = true)]
    mihomo: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Create directories and a deliberately non-operational example profile.
    Init,
    /// Gather local read-only evidence; this does not prove VPN authentication.
    Status(DiagnosticArgs),
    /// Diagnose local networking without changing system settings.
    Doctor(DiagnosticArgs),
    /// List and validate campus profiles.
    Profile {
        #[command(subcommand)]
        command: Profiles,
    },
    /// Build a separate config or validate existing YAML.
    Config {
        #[command(subcommand)]
        command: Configs,
    },
    /// Start trusted Mihomo and explicitly change the current user's system proxy.
    Start {
        #[arg(long, value_parser = ["system-proxy"])]
        mode: String,
        #[arg(long)]
        config: PathBuf,
    },
    /// Recover saved proxy state conditionally and terminate only the owned child.
    Stop,
}
#[derive(clap::Args)]
struct DiagnosticArgs {
    /// Profile identifier or YAML file used for route and detection checks.
    #[arg(long)]
    profile: Option<String>,
    /// Inspect this Mihomo YAML for Fake-IP overlap (never run it).
    #[arg(long)]
    config: Option<PathBuf>,
    /// Create a redacted, allowlisted JSON report; existing files are refused.
    #[arg(long)]
    export: Option<PathBuf>,
}
#[derive(Subcommand)]
enum Profiles {
    /// List profile filenames without exposing profile contents.
    List,
    /// Check schema, domain rules, CIDRs and supported routing mode.
    Validate { profile: String },
}
#[derive(Subcommand)]
enum Configs {
    /// Prepend campus DIRECT rules to a copy of the supplied base YAML.
    Build {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        input: PathBuf,
    },
    /// Check YAML structure and invoke mihomo -t if a trusted executable is found.
    Validate {
        #[arg(long)]
        config: PathBuf,
    },
}
fn run(cli: Cli) -> Result<()> {
    let home = storage::home(cli.home)?;
    match cli.command {
        Commands::Init => storage::init(&home)?,
        Commands::Status(args) | Commands::Doctor(args) => {
            let p = args
                .profile
                .as_deref()
                .map(|s| profile::resolve(&home, s).and_then(|p| profile::Profile::load(&p)))
                .transpose()?;
            diagnostics::run(
                &home,
                p.as_ref(),
                args.config.as_deref(),
                cli.mihomo.as_deref(),
                args.export.as_deref(),
            )?;
        }
        Commands::Profile {
            command: Profiles::List,
        } => {
            ensure!(home.join("profiles").is_dir(), "Run trucli init first");
            let mut entries =
                std::fs::read_dir(home.join("profiles"))?.collect::<std::io::Result<Vec<_>>>()?;
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                if e.path().extension().is_some_and(|s| s == "yaml") {
                    println!("{}", e.file_name().to_string_lossy());
                }
            }
        }
        Commands::Profile {
            command: Profiles::Validate { profile },
        } => {
            let p = profile::Profile::load(&profile::resolve(&home, &profile)?)?;
            println!("Profile valid: {} campus rules", p.rules().len());
        }
        Commands::Config {
            command: Configs::Build { profile, input },
        } => {
            let _lock = storage::lock(&home)?;
            let p = profile::Profile::load(&profile::resolve(&home, &profile)?)?;
            let generated = config::build(&home, &p, &input)?;
            mihomo::validate(&home, &generated, cli.mihomo.as_deref(), false)?;
        }
        Commands::Config {
            command: Configs::Validate { config },
        } => {
            mihomo::validate(&home, &config, cli.mihomo.as_deref(), false)?;
        }
        Commands::Start { mode: _, config } => {
            runtime::start(&home, &config, cli.mihomo.as_deref())?
        }
        Commands::Stop => runtime::stop(&home)?,
    }
    Ok(())
}
fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
