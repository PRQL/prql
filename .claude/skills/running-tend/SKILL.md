# Running Tend in PRQL

Tend-specific guidance for this repo. Project build commands, test strategy,
error conventions, etc. are in `CLAUDE.md` — don't duplicate them here.

## PR conventions

- PR titles use conventional commits: `feat:`, `fix:`, `docs:`, `chore:`,
  `refactor:`, `test:`, `ci:`, `internal:`, `devops:`, `web:`, `refine:`
- No scope required (e.g., `fix: resolve date parsing` not `fix(parser): ...`)
- Dependabot PRs use `chore:` prefix

## CI structure

- Main CI workflow: `tests` (watched by tend-ci-fix)
- Dependency management: Dependabot (tend-weekly is disabled)
- Automerge: `pull-request-target.yaml` auto-merges single-commit `prql-bot` PRs
  once CI passes

## Issue management

- Close bot-opened issues once the underlying cause is resolved — don't leave
  them open for a maintainer. If you (prql-bot) filed an issue (e.g., a nightly
  "tests failed" issue, a code-quality issue, an infra/upstream bug report) and
  the fix has merged or the upstream problem has been addressed, close the issue
  with a short comment citing the resolution (e.g., "Resolved by #NNNN —
  closing"). Applies to any issue where `author.login == prql-bot`.
