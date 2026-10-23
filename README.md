# Trucli

Trucli is an experimental Windows CLI that coordinates the official aTrust client
with Mihomo-based campus traffic routing.

**v0.1.0-alpha.1 · Windows 10/11 x86-64 · Experimental / Alpha**

[简体中文](README_zh-CN.md) · [Releases](https://github.com/styayur/trucli/releases)
· [Security](SECURITY.md) · [Changelog](CHANGELOG.md)

## What works

- Initialize user-owned state under `%LOCALAPPDATA%\Trucli`.
- Validate editable, institution-independent profiles with exact domains, domain
  suffixes, IPv4/IPv6 CIDRs, detection hints and optional DNS expectations.
- Prepend campus `DIRECT` rules to a separate copy of an existing Mihomo YAML.
  Preserve other fields and rule ordering. Resolve provider paths against the
  original directory and snapshot their inputs/caches into Trucli-owned storage. Comments and formatting are not retained.
- Run `mihomo -t -d <Trucli engine directory> -f <config>` when the trusted
  executable is available; clearly report structural-only validation otherwise.
- Gather read-only Windows process, installed-component, adapter, route, DNS,
  gateway, metric, proxy and runtime evidence. Export an allowlisted JSON summary.
- Explicitly start/stop a loopback Mihomo listener with conditional system proxy
  restoration and child identity checks.

**Limitations:** no TUN, aTrust login automation, reachability/authentication
verification, subscriptions UI or background watchdog. Only applications that
respect the current user's Windows system proxy are covered. Neither detection
nor generated `DIRECT` rules establishes VPN connectivity. Live campus/aTrust
compatibility has not been verified. Release executables are unsigned.

## Quick start

Install the official aTrust client through your institution and authenticate using
that client. Obtain an authorized, trusted Windows Mihomo proxy-engine executable
from [MetaCubeX's releases](https://github.com/MetaCubeX/mihomo/releases) and prepare
your own working Mihomo YAML. The engine source is on the
[`Meta` branch](https://github.com/MetaCubeX/mihomo/tree/Meta); inspect official
release provenance before executing a download. Trucli downloads neither engine.

Download the [alpha release](https://github.com/styayur/trucli/releases/tag/v0.1.0-alpha.1),
verify `SHA256SUMS.txt` using `Get-FileHash`, and extract the archive. In PowerShell:

```powershell
.\trucli.exe --version
.\trucli.exe init
notepad "$env:LOCALAPPDATA\Trucli\profiles\example.yaml"
```

Replace all `.invalid` placeholders with approved institutional resource domains.
Add only documented campus CIDRs, in canonical network form (no `/0` or host bits).
There is no universal campus address range. Keep `mode: system-proxy` and
`require_campus_rules: true`. The example intentionally fails validation until
edited. Optional `dns_expectations` are observations to check, not DNS changes.

The base YAML must use rule mode (or omit `mode`) and provide a nonzero `mixed-port`
or HTTP `port`. For startup, set `allow-lan: false`, `bind-address: 127.0.0.1`, and
disable TUN and the built-in DNS server. Omit controllers, custom listeners,
tunnels, transparent proxy ports, authentication, global `interface-name`, NTP and UI
fields. Trucli rejects these unsupported settings instead of silently changing
their meaning. Configure campus DNS resolution through your authorized network
setup; Trucli does not invent DNS servers. An optional SOCKS port must also be free.

```powershell
.\trucli.exe profile list
.\trucli.exe profile validate example
.\trucli.exe status --profile example
.\trucli.exe doctor --profile example --config .\mihomo.yaml
.\trucli.exe --mihomo 'C:\Tools\mihomo.exe' config build --profile example --input .\mihomo.yaml
# Use the generated path printed by build:
.\trucli.exe --mihomo 'C:\Tools\mihomo.exe' config validate --config '<generated.yaml>'
.\trucli.exe --mihomo 'C:\Tools\mihomo.exe' start --mode system-proxy --config '<generated.yaml>'
.\trucli.exe stop
```

`--mihomo` is optional when `mihomo.exe` is on an absolute PATH entry. Explicit
paths must identify an existing `.exe`; users are responsible for trusting it.
File providers are snapshots: rebuild to pick up changes. HTTP provider refreshes
write Trucli-owned caches. External validators may access provider URLs and create engine data; `status`
and `doctor` never invoke them. Validation output is suppressed to protect secrets;
run the engine's validator privately to investigate a failure.

Use `--help` on any command. `--home <directory>` isolates profiles/runtime state.
`init` preserves existing profiles; a build never overwrites its source or an
existing generated file. Rebuild after editing a generated configuration because
`start` checks its recorded SHA-256. Exit codes: **0** success (including explicitly
reported structural-only validation), **1** operational/validation failure,
**2** command syntax error. A proxy conflict on stop returns 1 after preserving the
newer settings and stopping the owned child.

## How routing and evidence work

Exact domain, domain-suffix and `IP-CIDR`/`IP-CIDR6` `DIRECT` rules are placed before
the base rules, including catch-all rules. Remaining traffic follows the base's
proxy groups and rules. Local-network access follows explicit profile/base rules.
Trucli creates no VPN gateway or routes: campus traffic can use existing OS VPN
routes only when those routes and the resource authorization already exist.

Detection uses user-supplied substring hints, visible installed aTrust entries,
and matching interfaces. Route evidence requires a non-default route covering a
configured campus CIDR on an up interface matching a hint. It can support
**possible connection established**, but never **authenticated**. Domain-only
profiles cannot provide CIDR route evidence. DNS, process names and virtual
adapters can all be ambiguous; reports expose evidence and uncertainty separately.
Routes are capped at 4096 observations; adapter metrics and default-route counts
are summarized without exporting gateways or identifying addresses.

Fake-IP overlap checks include configured IPv4/IPv6 pools and Mihomo's default
IPv4 Fake-IP pool when enhanced mode is `fake-ip`. A detected campus overlap blocks
generation. `doctor --profile example --config <file>` reports it without modifying
anything. DNS correctness and actual resource access still need local verification.

## Security and privacy

`status` and `doctor` only query Windows and files. No telemetry, credentials,
subscriptions, aTrust code or binaries are collected or distributed. Diagnostic
reports contain counts, flags and metrics rather than configuration values,
personal paths, resource names, addresses or tokens. To export a new local file:

```powershell
.\trucli.exe doctor --profile example --export .\trucli-report.json
```

`start` validates the owned generated config and trusted engine, records the
previous WinINet LAN proxy state, launches a suspended child, journals its PID,
creation time and executable path, then resumes it. The chosen HTTP listener must
be owned by that process before proxy activation. `stop` restores only if all
observed proxy settings still match Trucli's applied state (or are already
restored). Termination rechecks process identity on the same native handle.
User/app changes take precedence. WinHTTP, routes, DNS, firewall and aTrust state
are never changed. TUN is unsupported and is never a fallback.

Recovery metadata is local and includes the proxy snapshot, which may contain a
private PAC URL. Treat the directory and generated YAML as sensitive; they inherit
your Windows account's directory permissions. Protect them from other users and
do not publish them. The generated config can contain the base's proxy secrets.
Same-user malicious tampering and concurrent third-party proxy writers are outside
the security boundary. Compare/read/write operations cannot be atomic across apps.

After interrupted startup or unexpected engine exit, run `stop` with the same
`--home`; it works even if the owned engine has exited. Failed recovery retains
metadata for retry. There is no automatic crash watchdog or complete network
rollback guarantee. A hard crash between process creation and its identity journal
can leave a suspended child; inspect it locally rather than killing processes by
name. Preserve corrupt state for local investigation. See [SECURITY.md](SECURITY.md).

## Build and validation

Install Rust stable, the MSVC C++ build tools and the Windows SDK:

```powershell
cargo fmt --all -- --check
cargo check --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo install cargo-audit --version 0.22.2 --locked
cargo audit --deny warnings
cargo build --release --locked --target x86_64-pc-windows-msvc
python scripts/package.py
```

Python 3.11+ is needed only for packaging/inventory. Tests cover malformed
profiles/CIDRs, rule precedence and merge semantics, overlap, redaction, every help
surface, errors, initialization and mock proxy recovery. CI never changes a
runner's real proxy or routes. Native evidence and proxy reads are exercised by
local smoke checks; real system-proxy mutation and a campus session remain manual
verification work. [CI](https://github.com/styayur/trucli/actions/workflows/ci.yml)
and release workflows provide the actual check results.

## Releases and next steps

The tag workflow reruns checks, builds Windows x86-64, and publishes
`trucli-windows-x86_64.zip` with `SHA256SUMS.txt` as a prerelease. The archive
contains the unsigned executable, usage, license, dependency inventory and license
texts. Source for the matching tag is available from GitHub's source archives.
No aTrust or Mihomo executable is bundled. Compare the downloaded archive's
`Get-FileHash -Algorithm SHA256` result with the checksum file from that release.

Future work depends on field evidence: institution-specific opt-in compatibility
reports, broader DNS diagnostics and stronger crash recovery. TUN would require
separate privileged lifecycle design and testing; it has no compatibility claim.

## License and provenance

Copyright (C) 2026 styayur. Trucli is **AGPL-3.0-only**, without warranty, following
the [application policy](https://github.com/styayur/styayur/blob/main/LICENSE_POLICY.md).
See [LICENSE](LICENSE), [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) and
[DEPENDENCIES.json](DEPENDENCIES.json). Dependencies retain their own licenses.
Separately installed Mihomo and proprietary aTrust are not relicensed by Trucli.
LabVPN inspired module separation only; no source, assets or fdu-connect were used.
