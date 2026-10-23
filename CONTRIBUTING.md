# Contributing

Keep changes small, Windows-first and honest about evidence. Use synthetic
domains and documentation-only address ranges in tests. Never submit credentials,
real campus network details, subscription URLs or personal paths.

Rust stable and MSVC/Windows SDK are required. Before opening a PR, run:

```text
cargo fmt --all -- --check
cargo check --locked
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo audit --deny warnings
python scripts/package.py --check-inventory
```

When dependencies change, regenerate `DEPENDENCIES.json` with
`python scripts/package.py --inventory` and review their licenses and advisories.
Keep Cargo.lock committed. Add behavior-focused tests for meaningful changes.
Networking mutation tests must use mocks; never change CI runner proxy/routes.
Separate `platform/windows.rs` from pure profile/config/recovery decisions.

Explain the concrete problem, resulting behavior, validation and limits in PRs.
Update both READMEs when capabilities or limitations change. Use only authorized
official clients; do not add login bypasses, proprietary binaries or `fdu-connect`.
New source contributions are under the repository's AGPL-3.0-only license; retain
upstream notices for any separately licensed third-party work.

Main requires Windows checks and dependency auditing before merging. Use squash
merge only. The release maintainer verifies the archive, notices and checksums,
tags the checked main commit without overwriting existing tags, and waits for the
release workflow to publish the prerelease. Signing is not configured.
