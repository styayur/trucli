# Third-party notices and provenance

Trucli first-party source: Copyright (C) 2026 styayur, AGPL-3.0-only.
The authoritative application policy was inspected at
https://github.com/styayur/styayur/blob/main/LICENSE_POLICY.md before adoption.
No other project's license is changed.

## Rust dependencies

Exact versions, declared licenses and canonical source/repository links for every
resolved package are committed in [DEPENDENCIES.json](DEPENDENCIES.json).
Cargo.lock records registry checksums. This inventory includes target-specific and
build/proc-macro dependencies, even when not linked into the Windows executable.
`python scripts/package.py --inventory` regenerates it from `cargo metadata --locked`.
CI checks for drift. The release archive includes each dependency's available
LICENSE/COPYING/NOTICE texts under `dependency-licenses/<name>-<version>/`.
These are upstream texts, not first-party AGPL grants.

| Direct dependency | Purpose | Declared license |
| --- | --- | --- |
| anyhow | Contextual errors | MIT OR Apache-2.0 |
| clap | CLI parsing | MIT OR Apache-2.0 |
| ipnet | Canonical IPv4/IPv6 prefixes | MIT OR Apache-2.0 |
| serde | Typed serialization | MIT OR Apache-2.0 |
| serde_json | Runtime state and redacted reports | MIT OR Apache-2.0 |
| serde-saphyr | Bounded YAML parsing and semantic serialization | MIT OR Apache-2.0 |
| sha2 | Generated-file ownership digest | MIT OR Apache-2.0 |
| windows-sys | Official Win32 bindings (Windows only) | MIT OR Apache-2.0 |

Transitive licenses are itemized in the inventory. Standard Rust/Windows system libraries and toolchain components retain
their own terms; build tools are not shipped as separate binaries.

## External executables (not bundled)

**Mihomo proxy engine:** https://github.com/MetaCubeX/mihomo/tree/Meta

The `Meta` engine branch was inspected at commit
`3f27ef39f8c96bbfbc8b0b9bb10f5c9ef60149d8`. Its LICENSE is GNU GPL version 3:
https://github.com/MetaCubeX/mihomo/blob/3f27ef39f8c96bbfbc8b0b9bb10f5c9ef60149d8/LICENSE
Its main.go implements `-t`, `-f` and `-d`, used through a separate process.
The repository's default branch/metadata at inspection described an unrelated
Python project with an MIT license; that metadata is not used to license the Go
proxy engine. Users must verify the specific engine version they obtain from
https://github.com/MetaCubeX/mihomo/releases and its accompanying source/license.
Official configuration reference: https://wiki.metacubex.one/
Trucli does not download, copy, link or redistribute this engine. Its license
does not become AGPL merely because Trucli launches it.

**Sangfor aTrust:** proprietary, separately installed and authorized through the
user's institution or vendor. Obtain it only through those official channels.
Trucli does not redistribute its binaries, drivers, libraries or installer;
does not implement its authentication protocol; and does not possess its source
or redistribution rights. It remains subject to the vendor's license.

## Architectural reference

LabVPN-client: https://github.com/echo-edai/LabVPN-client
Inspected commit: `f490f3df7b92a4969eac5b4f101eff8d8011ff41`.
MIT License, Copyright (c) 2026 LabVPN Contributors.
License: https://github.com/echo-edai/LabVPN-client/blob/f490f3df7b92a4969eac5b4f101eff8d8011ff41/LICENSE

Its architecture documentation informed the separation between pure profile/rule
logic and platform/process operations. No source, UI assets, configuration presets,
institution-specific ranges or dependency graph were copied. No `fdu-connect`
code or executable is incorporated. This is an original, independent CLI.
