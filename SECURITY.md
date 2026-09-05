# Security Policy

## Supported Versions

| Version | Supported |
| :------ | :-------- |
| Latest release | ✅ |

Only the latest published release receives security fixes. Fixes are
released as a patch bump and announced in the [CHANGELOG](CHANGELOG.md).

## Reporting a Vulnerability

Please **do not** open a public issue for security vulnerabilities.

Instead, report them privately via GitHub's
[Security Advisories](https://github.com/swadhinbiswas/warren/security/advisories/new)
feature, or email the maintainer through the contact details on their
[GitHub profile](https://github.com/swadhinbiswas).

Please include:

- The affected version(s)
- A description of the vulnerability
- Steps to reproduce (including any crafted input, e.g. a malicious
  `.tar.gz` archive for `warren import`)
- Impact assessment

You will receive a response within 7 days. We appreciate responsible
disclosure and will credit reporters in the advisory unless anonymity
is requested.

## Design Assumptions

- Warren runs **as the current user, never as root**. It refuses to start
  with euid 0.
- Installer scripts executed by `warren dig`/`warren update` are untrusted
  code and are run with the caller's privileges, inside a rewritten
  environment. Review scripts before running them.
- `warren import` validates archive entries and rejects path traversal
  before extracting; if you find a way around that, report it immediately.
