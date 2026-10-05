# Publication and security review

Volt was published on 2026-10-05 as a **new repository with cleaned Git
history**. The original repository, including its old pull requests, releases,
and Actions logs, remains a private, read-only archive. Reusing its former URL
does not migrate those GitHub records to this repository.

## What was reviewed

- The published `main` initially had the same source tree as the private
  archive's `main`. Historical `target/` build output and local agent-tooling
  files were excluded from the published history. Rewriting history changed
  commit and tag IDs; do not push old private refs into this repository.
- Gitleaks v8.30.1 found no secrets in the reachable cleaned history. The
  current tracked tree was also checked for credential files. These checks
  cannot prove that no secret exists.
- The private archive's retrievable Actions logs and a sample of historical
  compiled objects were checked for recognizable credentials. Some old job
  logs were no longer retrievable, so that historical audit is incomplete;
  those logs and objects were **not** published here.

## Ongoing safeguards

- `main` requires a pull request and passing formatting, Clippy, tests, MSRV,
  dependency audit, and full-history secret-scan checks. The rule applies to
  administrators and blocks force-pushes and deletion. Approval count is zero
  while Volt has a solo maintainer; this is a PR gate, not an independent code
  review.
- GitHub secret-scanning alerts, push protection, Dependabot security updates,
  and private vulnerability reporting are enabled.
- Report suspected vulnerabilities privately as described in
  [SECURITY.md](../SECURITY.md). If a credential is ever found, rotate it;
  removing it from Git history is not a substitute for rotation.

These controls reduce risk; they are not a guarantee that the terminal, its
dependencies, or its release artifacts are free of vulnerabilities.
