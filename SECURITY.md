# Security Policy

## Supported Versions

libmagic-rs is currently in active development as a passion project. Security updates are provided on a best-effort basis for the following versions:

| Version | Supported          | Notes |
| ------- | ------------------ | ----- |
| 1.0.x   | :white_check_mark: | Production releases (when available) |
| 0.3.x   | :white_check_mark: | Pre-release with security fixes |
| 0.2.x   | :white_check_mark: | Pre-release with security fixes |
| 0.1.x   | :white_check_mark: | MVP releases with security fixes |
| < 0.1   | :x:                | Development versions |

## Security Considerations

### Memory Safety

- **Pure Rust Implementation**: No unsafe code except in vetted dependencies
- **Bounds Checking**: All buffer access protected by bounds checking
- **Safe File Handling**: Graceful handling of truncated/corrupted files
- **Fuzzing Integration**: Robustness testing with malformed inputs

### Input Validation

- **Magic File Validation**: Syntax validation before parsing
- **File Size Limits**: Protection against resource exhaustion
- **Malformed Input Handling**: Safe processing of corrupted files
- **Timeout Protection**: Configurable timeouts for long-running evaluations

### Dependencies

- **Vetted Dependencies**: Only trusted, well-maintained crates
- **Security Audits**: Regular `cargo audit` checks
- **Minimal Attack Surface**: Minimal external dependencies
- **License Compliance**: All dependencies must have compatible licenses

## Reporting a Vulnerability

### How to Report

If you discover a security vulnerability in libmagic-rs, please report it responsibly:

1. **Email**: Send details to <security@evilbitlabs.com>
2. **GitHub Security**: Use GitHub's private vulnerability reporting feature
3. **Do NOT**: Open public issues for security vulnerabilities

### What to Include

- Description of the vulnerability
- Steps to reproduce the issue
- Potential impact assessment
- Suggested fix (if any)
- Your contact information for follow-up

### Response Timeline

**Note**: This is a passion project with volunteer maintainers. Response times are best-effort and may vary based on maintainer availability.

- **Acknowledgment**: Best effort (typically within 1 week)
- **Initial Assessment**: Best effort (typically within 2 weeks)
- **Fix Development**: Best effort (timeline depends on severity and maintainer availability)
- **Public Disclosure**: Coordinated with fix release when possible

### Severity Levels

- **Critical**: Remote code execution, memory corruption
- **High**: Denial of service, information disclosure
- **Medium**: Limited information disclosure, minor DoS
- **Low**: Minor issues, edge cases

## Security Best Practices

### For Users

- Keep libmagic-rs updated to the latest version
- Validate input files before processing
- Use appropriate file size limits
- Monitor for security advisories

### For Developers

- Follow Rust security best practices
- Use `cargo audit` regularly
- Implement proper error handling
- Test with malformed inputs
- Review all unsafe code usage

## Security Acknowledgments

We appreciate responsible disclosure and will acknowledge security researchers who help improve libmagic-rs security. Contributors will be listed in our security acknowledgments (with permission).

**Note**: As a passion project, we may not always be able to provide immediate responses or fixes, but we do our best to address security issues when maintainers are available.

## Contact

For security-related questions or concerns:

- **Email**: <security@evilbitlabs.com>
- **GitHub**: [Security Advisories](https://github.com/EvilBit-Labs/libmagic-rs/security/advisories)
- **Issues**: Use private vulnerability reporting for security issues
