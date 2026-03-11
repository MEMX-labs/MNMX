# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | Yes                |
| < 0.1   | No                 |

As MNMX Core matures, this table will be updated to reflect the current
support window. Only the latest minor release receives security patches.

## Reporting a Vulnerability

If you discover a security vulnerability in MNMX Core, please report it
responsibly. **Do not open a public GitHub issue.**

### Contact

Send your report to:

**security@mnmx.io**

Include the following information:

1. **Description**: A clear explanation of the vulnerability.
2. **Reproduction steps**: Minimal steps or code to reproduce the issue.
3. **Impact assessment**: Your analysis of what an attacker could achieve.
4. **Affected versions**: Which versions you have tested against.
5. **Suggested fix** (optional): If you have a patch or mitigation in mind.

### What to Expect

- **Acknowledgment**: We will confirm receipt of your report within 48 hours.
- **Triage**: Within 5 business days we will assess severity and assign a
  tracking identifier.
- **Updates**: We will keep you informed of progress at least once per week
  until the issue is resolved.
- **Resolution**: We aim to release a patch within 14 days for critical
  vulnerabilities and within 30 days for lower-severity issues.

### Disclosure Process

1. Reporter submits the vulnerability via the contact method above.
2. The MNMX security team triages and reproduces the issue.
3. A fix is developed and tested in a private branch.
4. A new release is published with the fix.
5. A security advisory is published on the repository (GitHub Security
   Advisories) after the fix is available.
6. The reporter is credited in the advisory unless they request anonymity.

### Scope

The following are in scope:

- The `@mnmx/core` npm package and all code in this repository.
- Logic errors in the minimax engine that could lead to incorrect execution
  plans or financial loss.
- Vulnerabilities in transaction construction or signing flows.
- Dependency vulnerabilities that affect MNMX Core in practice.

The following are out of scope:

- Vulnerabilities in third-party dependencies that do not affect MNMX Core.
- Issues in example code or documentation that do not represent production
  usage patterns.
- Social engineering attacks against maintainers.

### Safe Harbor

We consider security research conducted in good faith to be authorized. We
will not pursue legal action against researchers who:

- Make a good-faith effort to avoid privacy violations, data destruction, and
  service disruption.
- Report vulnerabilities through the process described above.
- Allow reasonable time for a fix before public disclosure.

## General Security Practices

- All dependencies are monitored via Dependabot for known vulnerabilities.
- The CI pipeline runs on every pull request to catch regressions.
- Sensitive operations (transaction signing, key management) are isolated
  and never logged.
