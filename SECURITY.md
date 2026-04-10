# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.5.x   | :white_check_mark: |
| < 0.5   | :x:                |

Users on unsupported versions should upgrade to the latest release. Please review the [release notes](https://github.com/EvilBit-Labs/libmagic-rs/releases) when upgrading.

## Reporting a Vulnerability

We take the security of libmagic-rs seriously. If you believe you have found a security vulnerability, please report it to us as described below.

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, use one of the following channels:

1. [GitHub Private Vulnerability Reporting](https://github.com/EvilBit-Labs/libmagic-rs/security/advisories/new) (preferred)
2. Email [support@evilbitlabs.io](mailto:support@evilbitlabs.io) encrypted with our [PGP key](#pgp-key) (verify the full fingerprint below before use)

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

## PGP Key

**Fingerprint:** `F839 4B2C F0FE C451 1B11 E721 8F71 D62B F438 2BC0`

```text
-----BEGIN PGP PUBLIC KEY BLOCK-----

mDMEaLJmxhYJKwYBBAHaRw8BAQdAaS3KAoo+AgZGR6G6+m0wT2yulC5d6zV9lf2m
TugBT+O0L3N1cHBvcnRAZXZpbGJpdGxhYnMuaW8gPHN1cHBvcnRAZXZpbGJpdGxh
YnMuaW8+iNcEExYKAH8DCwkHRRQAAAAAABwAIHNhbHRAbm90YXRpb25zLm9wZW5w
Z3Bqcy5vcmexd21FpCDfIrO7bf+T6hH/8drbGLWiuEueWvSTyw4T/QMVCggEFgAC
AQIZAQKbAwIeARYhBPg5Syzw/sRRGxHnIY9x1iv0OCvABQJpiUiCBQkIXQE5AAoJ
EI9x1iv0OCvAm2sA/AqFT6XEULJCimXX9Ve6e63RX7y2B+VoBVHt+PDaPBwkAP4j
39xBoLFI6KZJ/A7SOQBkret+VONwPqyW83xfn+E7Arg4BGiyZsYSCisGAQQBl1UB
BQEBB0ArjU33Uj/x1Kc7ldjVIM9UUCWMTwDWgw8lB/mNESb+GgMBCAeIvgQYFgoA
cAWCaLJmxgkQj3HWK/Q4K8BFFAAAAAAAHAAgc2FsdEBub3RhdGlvbnMub3BlbnBn
cGpzLm9yZ4msIB6mugSL+LkdT93+rSeNePtBY4Aj+O6TRFU9aKiQApsMFiEE+DlL
LPD+xFEbEechj3HWK/Q4K8AAALEXAQDqlsBwMP2XXzXDSnNNLg8yh1/zQcxT1zZ1
Z26lyM7L6QD+Lya5aFe74WE3wTys5ykGuWkHYEgba+AyZNmuPhwMGAc=
=9zSi
-----END PGP PUBLIC KEY BLOCK-----
```

## Contact

For general security questions, open a GitHub Issue. For vulnerability reports, use [Private Vulnerability Reporting](https://github.com/EvilBit-Labs/libmagic-rs/security/advisories/new) or email [support@evilbitlabs.io](mailto:support@evilbitlabs.io).

---

Thank you for helping keep libmagic-rs and its users secure!
