# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

Users on unsupported versions should upgrade to the latest release. Please review the [release notes](https://github.com/EvilBit-Labs/libmagic-rs/releases) when upgrading.

## Reporting a Vulnerability

We take the security of libmagic-rs seriously. If you believe you have found a security vulnerability, please report it to us as described below.

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, use one of the following channels:

1. [GitHub Private Vulnerability Reporting](https://github.com/EvilBit-Labs/libmagic-rs/security/advisories/new) (preferred)
2. Email [security@evilbitlabs.com](mailto:security@evilbitlabs.com)

Please include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### Scope

**In scope:**

- Buffer overflows or out-of-bounds reads in magic file parsing or evaluation
- Denial of service via crafted magic files or input files
- Path traversal in file input handling
- Command injection via CLI arguments
- Unsafe code in dependencies that affects libmagic-rs

**Out of scope:**

- Vulnerabilities in the original C libmagic implementation
- Issues requiring physical access to the machine running libmagic-rs
- Social engineering attacks

### What to Expect

**Note**: This is a passion project with volunteer maintainers. Response times are best-effort and may vary based on maintainer availability.

- We will acknowledge receipt of your report within **1 week**
- We will provide an initial assessment within **2 weeks**
- We aim to release a fix within **90 days** of confirmed vulnerabilities
- We will coordinate disclosure through a [GitHub Security Advisory](https://github.com/EvilBit-Labs/libmagic-rs/security/advisories)
- We will credit you in the advisory (unless you prefer to remain anonymous)

### Responsible Disclosure

We ask that you:

- Give us reasonable time to respond to issues before any disclosure
- Avoid accessing or modifying other users' data
- Avoid actions that could negatively impact other users

## Security Features

libmagic-rs includes several security-focused features:

- **Pure Rust implementation**: No unsafe code except in vetted dependencies
- **Bounds checking**: All buffer access protected by bounds checking
- **Safe file handling**: Graceful handling of truncated and corrupted files
- **Dependency auditing**: Regular `cargo audit` and `cargo deny` checks
- **Automated dependency updates**: Via Dependabot

## Safe Harbor

We support safe harbor for security researchers who:

- Make a good faith effort to avoid privacy violations, data destruction, and service disruption
- Only interact with accounts you own or with explicit permission of the account holder
- Report vulnerabilities through the channels described above

We will not pursue legal action against researchers who follow this policy.

## Contact

For general security questions, open a GitHub Issue. For vulnerability reports, use [Private Vulnerability Reporting](https://github.com/EvilBit-Labs/libmagic-rs/security/advisories/new) or email [security@evilbitlabs.com](mailto:security@evilbitlabs.com).

---

Thank you for helping keep libmagic-rs and its users secure!
