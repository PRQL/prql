# Running Tend in PRQL

Tend-specific guidance for this repo. Project build commands, test strategy,
error conventions, etc. are in `CLAUDE.md` — don't duplicate them here.

## Filing issues in other repos

Standing exception granted: file directly in agent-equipped targets (per
**Filing Issues in Other Repos** in the bundled `running-in-ci` skill) without
asking permission here first. The default rule (open an issue here asking
permission first) still applies when the target shows no agent signals.

## PR conventions

- PR titles use conventional commits: `feat:`, `fix:`, `docs:`, `chore:`,
  `refactor:`, `test:`, `ci:`, `internal:`, `devops:`, `web:`, `refine:`
- No scope required (e.g., `fix: resolve date parsing` not `fix(parser): ...`)
- Dependabot PRs use `chore:` prefix

## CI structure

- Main CI workflow: `tests` (watched by tend-ci-fix)
- Dependency management: Dependabot opens dependency PRs; tend-weekly reviews
  them and runs the tasks under Weekly maintenance below.
- Automerge: not configured — `pull-request-target.yaml` only validates PR
  titles and handles `pr-backport-web` backports. The automerge job was removed
  in #5753, so bot PRs must be merged manually by a maintainer (or via repo
  branch-protection auto-merge if a maintainer enables it on the PR).

## CI monitoring: poll in sub-cap chunks

The bundled `running-in-ci` CI-monitoring recipe runs a single foreground loop
of 15 one-minute iterations (`for i in $(seq 1 15); do sleep 60`). That
15-minute loop exceeds the Bash tool's 10-minute cap, so the harness
auto-backgrounds it — and a backgrounded poll's completion notification is not
reliably delivered to a CI session (see
[max-sixty/tend#694](https://github.com/max-sixty/tend/issues/694), still open
as of tend 0.1.6). When the wait is gated (a review approval/dismissal, a
pushed-fix verification), the session then waits on the backgrounded task for a
notification that never arrives and ends without posting the gated action. This
already cost a deliverable: the #6022 dependabot review deadlocked on the
auto-backgrounded poll and posted no review at all.

Until #694 is fixed upstream, run the poll in chunks that each stay under the
cap: cap the loop at **8 iterations** (`seq 1 8`) per Bash call, and if checks
are still pending when it returns, issue another Bash call to keep polling. Every
poll stays in the foreground — never wait on a backgrounded poll to notify you.

## Weekly maintenance

These tasks run as Step 3 of the bundled weekly skill (only when
`workflows.weekly` is enabled in `.config/tend.yaml`).

- **Bump pinned `go-task/setup-task` version.** The action is invoked with a
  concrete `version:` input to avoid the intermittent
  `unable to get latest version` failure from `version: 3.x` (see #5836).
  Dependabot does not update `with:` inputs, so this needs a manual weekly bump.
  Find the latest release at <https://github.com/go-task/task/releases/latest>;
  if the current pin is older, update `version: X.Y.Z` in:
  - `.github/actions/tend-setup/action.yaml`
  - `.github/workflows/build-web.yaml`
  - `.github/workflows/test-php.yaml`
  - `.github/workflows/test-prqlc-c.yaml`

  Open a single `chore:` PR with the bump. Skip if already at the latest.

## Issue management

- Close bot-opened issues once the underlying cause is resolved — don't leave
  them open for a maintainer. If you (prql-bot) filed an issue (e.g., a nightly
  "tests failed" issue, a code-quality issue, an infra/upstream bug report) and
  the fix has merged or the upstream problem has been addressed, close the issue
  with a short comment citing the resolution (e.g., "Resolved by #NNNN —
  closing"). Applies to any issue where `author.login == prql-bot`.
