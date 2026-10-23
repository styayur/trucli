# Changelog

## [0.1.0-alpha.1] - 2026-10-24

First experimental Windows 10/11 x86-64 CLI release.

- Initialize user-owned profiles; validate domains, canonical IPv4/IPv6 CIDRs and hints.
- Build separate Mihomo configurations with campus DIRECT rules before base rules.
- Perform structural validation and invoke a separately installed trusted Mihomo validator when available.
- Gather read-only aTrust/network evidence and export allowlisted diagnostic summaries.
- Start loopback system-proxy mode with journaled child identity and conditional proxy restoration on stop.
- Add mock recovery tests, configuration/redaction/CLI tests, Windows CI, dependency auditing and release packaging with notices/checksums.

Limitations: unsigned alpha executable; no TUN, login automation, authentication or
reachability verification, automatic engine installation or crash watchdog.
System-proxy mutation and real campus/aTrust sessions have not been manually
verified. Detection reports evidence rather than confirmed connectivity. See
README for unsupported base-config fields and recovery limitations.
