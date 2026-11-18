# Security

## Reporting a vulnerability

The crate reads untrusted files: a panic, a hang, an excessive allocation or a wrong result on
a crafted input is a bug we want to know about. Please report it privately through
[GitHub security advisories](https://github.com/ybdn/ansi-nist-itl/security/advisories/new)
rather than in a public issue.

## Never share real NIST files

ANSI/NIST-ITL files hold fingerprints, faces and identities. **Do not attach a real file to an
issue, a pull request or an advisory**, even redacted. Instead:

- give the error message and its position (record, field, byte offset), which every error
  carries;
- or reproduce the problem with a synthetic file built by the crate (or by `nistitl`) with
  invented values;
- or minimize the input with the fuzz tooling (`cargo fuzz tmin`) and check that what is left
  holds no personal data.

## Supported versions

Fixes go to the latest release and to `develop`.
