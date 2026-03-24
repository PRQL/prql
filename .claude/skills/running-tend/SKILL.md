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
- Dependency management: Dependabot (tend-renovate is disabled)
- Automerge: `pull-request-target.yaml` auto-merges single-commit `prql-bot` PRs
  once CI passes
