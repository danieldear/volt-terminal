# Public-repository readiness

Keep this repository private until its history and GitHub Actions logs have
been reviewed. Making it public exposes **all reachable Git history**, tags,
and Actions history/logs, not only the files currently on `main`.

## Current audit (2026-10-05)

- Gitleaks v8.30.1 reported no findings when scanning all reachable commits.
- The current tracked tree contains no credential files; the old agent-tooling
  files were removed from `main`, but remain in Git history. The historical
  `.mcp.json` config contains an agent007 command and no environment values.
- An early commit tracked `target/` build output: 8,868 historical paths. The
  repository's packed object store is about 708 MiB. Some historical compiled
  objects embed local home-directory paths. A clean public-history option
  should remove these artifacts before publication.
- An **unpushed local preview** removing `target/` and the old agent-tooling
  files retained all 89 commits and the exact current `main` source tree while
  reducing the packed object store to about 2.4 MiB. Its commit and tag IDs
  differ from the private repository's, so it must not be force-pushed over
  existing releases without a separate migration decision.
- Gitleaks reported no findings in the logs retrievable for 155 of 161 Actions
  runs, plus 5 additional job logs from the remaining runs. GitHub no longer
  served 26 job logs, so those cannot be cleared by this audit.
- A targeted byte-pattern check of 84 historical first-party compiled/build
  objects found no recognizable private-key, GitHub, OpenAI-style, AWS, or
  Slack credential patterns, but 48 objects contained local home-directory
  paths. Other compiled objects were not exhaustively inspected.
- Scanners cannot prove there are no secrets. Rotate any credential if a later
  review discovers exposure; deleting it from Git is not a substitute for
  rotation.

## Before changing visibility

1. Prefer a fresh public repository from a reviewed source snapshot, keeping
   this private repository and its releases/PR history unchanged. If retaining
   the same repository is essential, plan a coordinated history rewrite of
   **all** branches and tags, then review release links and forks; do not
   force-push rewritten history casually.
2. Review Actions logs and release assets for sensitive values or private
   paths. Confirm no deploy/signing secrets are configured unintentionally.
3. Run the full-history secret scan, dependency audit, tests, and release build
   on the exact commit selected for publication.
4. After making the repository public, immediately protect `main`: require a
   pull request and passing `Rustfmt`, both `Clippy` jobs, both `Tests` jobs,
   `Rust 1.87 MSRV`, `audit`, and `Secret scan`; block force-pushes and deletion;
   enforce the rule for administrators. A solo maintainer can require PRs with
   zero approving reviews, adding review approval later when collaborators
   are available.
5. Enable GitHub secret-scanning alerts, push protection, and private
   vulnerability reporting. Verify the protection rule and alerts are active.

GitHub Free does not enforce branch protection on this private repository.
The repo must be made public (or the account upgraded) before its `main`
protection can be enabled. Do not mistake CI checks for branch protection.
