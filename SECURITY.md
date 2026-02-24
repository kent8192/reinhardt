# Security Policy

## Supported Versions

Currently, only the latest development version is supported. Stable releases are not yet available.

| Version | Supported |
|---------|-----------|
| Latest development branch | ✅ Yes |
| Other branches | ❌ No |

**Note:** This project is in active development and has not yet released a stable version.

---

## Reporting a Vulnerability

### Private Disclosure Process

**Security vulnerabilities MUST be reported privately.**

**DO NOT:**
- Create public issues for security vulnerabilities
- Discuss vulnerabilities in public channels (Discussions, Discord, etc.)
- Post vulnerability details on social media

**DO:**
- Report vulnerabilities via GitHub Security Advisories (preferred)
- Include reproduction steps and impact assessment
- Allow time for assessment and patching

---

## How to Report

### Method 1: GitHub Security Advisories (Recommended)

Use GitHub's built-in private vulnerability reporting:

```
https://github.com/kent8192/reinhardt-web/security/advisories
```

Click "Report a vulnerability" and fill in the form.

### Method 2: Contact Maintainers

If you cannot use GitHub Security Advisories, contact the repository maintainers directly through GitHub's private messaging system.

---

## What to Include

Your report should include:

1. **Vulnerability Description**
   - Type of vulnerability (XSS, SQL injection, etc.)
   - Affected components
   - Impact assessment

2. **Affected Versions**
   - Specific commit hashes or version numbers
   - Branch names if applicable

3. **Steps to Reproduce**
   - Minimal reproduction code
   - Configuration details
   - Prerequisites

4. **Proof of Concept (Optional but Recommended)**
   - Working exploit demonstration
   - Screenshots or logs

5. **Proposed Mitigation (Optional)**
   - Suggested fix or workaround
   - Additional security considerations

---

## Vulnerability Response Process

### Timeline

| Phase | Duration | Description |
|-------|----------|-------------|
| **Acknowledgment** | Within 48 hours | Initial confirmation that report was received |
| **Assessment** | Within 7 days | Severity classification and impact analysis |
| **Patch Development** | 7-30 days | Fix development and testing (varies by severity) |
| **Release** | After patch ready | Coordinated public disclosure |

### Response Workflow

1. **Acknowledgment (Within 48 hours)**
   - Private security advisory created
   - Issue labeled `security` and `critical`
   - Maintainer assigned

2. **Assessment (Within 7 days)**
   - Vulnerability confirmed and classified
   - Severity level assigned (Critical, High, Medium, Low)
   - Impact analysis completed
   - Fix strategy determined

3. **Patch Development (7-30 days)**
   - Critical: 7 days
   - High: 14 days
   - Medium: 21 days
   - Low: 30 days
   - Private fix developed and tested

4. **Coordinated Disclosure**
   - Maintainer contacts reporter
   - Release date agreed upon
   - Security advisory published
   - Public disclosure after fix is released

---

## Severity Levels

### Critical (CVSS 9.0-10.0)

- Remote code execution without authentication
- SQL injection in core functionality
- Authentication bypass
- Data integrity compromise

**Response Time:** 7 days

### High (CVSS 7.0-8.9)

- SQL injection in non-core functionality
- Privilege escalation
- Sensitive data exposure
- DoS vulnerability affecting availability

**Response Time:** 14 days

### Medium (CVSS 4.0-6.9)

- XSS vulnerabilities
- CSRF vulnerabilities
- Local file inclusion
- Information disclosure

**Response Time:** 21 days

### Low (CVSS 0.1-3.9)

- Minor security issues
- Best practice violations
- Low-risk information disclosure

**Response Time:** 30 days

---

## Coordinated Disclosure

### Process

1. **Reporter Submits** private vulnerability report
2. **Maintainer Acknowledges** within 48 hours
3. **Assessment** completed within 7 days
4. **Patch Development** timeline set based on severity
5. **Release Coordination** with reporter
6. **Public Disclosure** after fix is released

### Disclosure Policy

- Vulnerabilities are disclosed publicly **after** a fix is released
- Credit is given to reporters (unless anonymous is requested)
- Security advisories include:
  - Vulnerability description
  - Affected versions
  - Patched versions
  - Mitigation steps
  - Acknowledgments

---

## Security Best Practices

### For Users

1. **Keep Dependencies Updated**
   - Regularly update Rust dependencies
   - Monitor security advisories for dependencies
   - Use `cargo audit` to check for vulnerabilities

2. **Input Validation**
   - Always validate user input
   - Use parameterized queries (reinhardt-query)
   - Sanitize output to prevent XSS

3. **Authentication & Authorization**
   - Use strong password hashing (bcrypt, Argon2)
   - Implement proper session management
   - Use HTTPS in production

4. **Secrets Management**
   - Never commit secrets to git
   - Use environment variables for configuration
   - Rotate secrets regularly

5. **Database Security**
   - Use prepared statements (reinhardt-query)
   - Implement principle of least privilege
   - Enable database connection encryption

### For Developers

1. **SQL Injection Prevention**
   - Use reinhardt-query for all SQL construction
   - Never concatenate SQL strings
   - Validate and sanitize all inputs

2. **Authentication**
   - Use industry-standard libraries
   - Implement secure password hashing
   - Use secure session management

3. **Authorization**
   - Check permissions on every request
   - Implement role-based access control
   - Use principle of least privilege

4. **Dependencies**
   - Keep dependencies updated
   - Review security advisories
   - Use `cargo-audit` in CI

5. **Error Handling**
   - Don't expose sensitive information in errors
   - Log security events appropriately
   - Monitor for suspicious activity

---

## Security Audits

This project has not yet undergone a formal security audit.

**Planned:**
- First audit before 1.0.0 release
- Annual audits thereafter
- Penetration testing for web components

---

## Receiving Security Updates

### Security Advisories

All security advisories will be published at:
```
https://github.com/kent8192/reinhardt-web/security/advisories
```

### Dependabot Alerts

Enable Dependabot alerts in your fork to receive automatic vulnerability notifications for dependencies.

### Monitoring

- Watch this repository for releases
- Subscribe to security advisory notifications
- Follow the project on GitHub for updates

---

## Security-Related Resources

- **Issue Guidelines**: docs/ISSUE_GUIDELINES.md (see Security Issues section)
- **Contributing Guide**: CONTRIBUTING.md
- **Code of Conduct**: CODE_OF_CONDUCT.md

---

## Acknowledgments

We thank all security researchers who responsibly disclose vulnerabilities to help improve the security of Reinhardt.

---

**Last Updated:** 2026-01-26

**Repository:** https://github.com/kent8192/reinhardt-web
