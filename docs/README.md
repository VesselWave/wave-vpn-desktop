# Documentation

## Using Wave VPN

- [User guide](USER-GUIDE.md): signing in, connection checks, Network Lock, Recover, upgrading and uninstalling
- [Installation and support](SUPPORT.md): tested systems, dependencies, diagnostics
- [v0.1.3 release notes](RELEASE-v0.1.3.md): infrastructure load, recovery and server selection
- [v0.1.2 release notes](RELEASE-v0.1.2.md): previous release
- [v0.1.1 release notes](RELEASE-v0.1.1.md): the interface update
- [v0.1.0 release notes](RELEASE-v0.1.0.md): the first release and its known limits
- [Security policy](../SECURITY.md): private reporting and the signing fingerprint

## How it was tested

Wave VPN publishes its test evidence, including failures. [Verification](VERIFICATION.md) summarizes what has been tested and what hasn't, and [release gates](RELEASE-GATES.md) list what still has to pass before a stable release.

| Report | What it covers |
| --- | --- |
| [Public prerelease preflight](qa/RELEASE-PUBLIC-PREFLIGHT.md) | Authenticated live suite on ARM64 from the published source |
| [Physical laptop baseline](qa/RELEASE-PHYSICAL-LAPTOP.md) | Shipped binary on physical CachyOS: GUI rendering, staging, 19-scenario network suite |
| [Desktop feature review](qa/DESKTOP-FEATURE-REVIEW.md) | UI and feature review against the scope |

## Project

- [Contributing](../CONTRIBUTING.md): building, the network test suites, architecture
- [Scope](PARITY.md): what the desktop app does and deliberately doesn't do
- [Provenance](PROVENANCE.md): which official references the protocol follows, and reused assets
- Release preparation: [public-release checklist](qa/PUBLIC-RELEASE-CHECKLIST.md) and [visibility review](qa/PUBLIC-VISIBILITY-REVIEW.md)
