# Security

Trucli v0.1.0-alpha.1 is experimental. It is not a security boundary, a VPN
implementation or a certified networking product. No stable release is supported.
Fixes target the latest alpha source; do not rely on this tool for critical access.

Report a suspected vulnerability privately using
[GitHub private reporting](https://github.com/styayur/trucli/security/advisories/new)
when available. If unavailable, open an issue containing only a request for a
private reporting channel; do not publish exploit details or private data.

Include version, Windows version, a minimal synthetic reproduction and impact.
Never include real credentials, subscription tokens, PAC URLs, personal paths,
raw runtime state or network dumps. `doctor --export` produces a count/flag/metric
summary; inspect it before sharing. There is no remote diagnostic upload.

`status` and `doctor` do not mutate networking or launch aTrust/Mihomo. Explicit
`start` changes current-user WinINet LAN proxy settings after validating a trusted
Mihomo and an owned generated YAML. No WinHTTP, route, DNS, firewall or aTrust
changes occur. Engine validation may access configured provider URLs or write
engine/provider caches; only execute configurations and engine binaries you trust.

Runtime recovery compares the full applied proxy snapshot before restoring the
previous snapshot. Stop checks PID, creation time and executable path using the
same native process handle before terminating the child. An OS file lock serializes
Trucli mutations, but cannot serialize other apps' proxy writes. Same-user
malicious tampering, replacement executables, concurrent external writers and
untrusted configuration providers are outside the threat boundary. Validation
is not a sandbox. The CLI does not require elevation for supported proxy mode.

Generated YAML may contain the user's existing proxy credentials. Runtime state
contains the minimum proxy snapshot needed to restore flags, server, bypass and
PAC URL, plus child identity. These files inherit local account permissions and
must stay private. No aTrust credentials are read or stored. Do not run Trucli
under an elevated/shared account or place its home directory in a shared location.

Unexpected engine exit can leave the proxy pointing at an inactive listener;
run `trucli stop` using the original home. Recovery failures retain state for retry.
If another app changed the proxy, stop preserves its newer settings. Corrupt state
is never guessed or reset. A hard crash before the child identity is journaled can
leave a suspended child; inspect its native identity locally rather than killing
by name. There is no promise of complete rollback or automatic crash recovery.

The first release is unsigned. Verify the release checksum and matching source
tag. Checksum agreement verifies file integrity against that publication; it is
not code signing or proof of security. CI uses mocks for system mutation and
does not establish real aTrust/campus compatibility.
