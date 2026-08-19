---
name: running-tend
description:
  PRQL-specific guidance for tend CI workflows. Adds a standing exception for
  filing issues in other repos, PR title conventions, CI structure,
  Dependabot-batch polling, weekly maintenance tasks, and issue-closing policy
  on top of the generic tend-* skills. Use when operating in CI.
---

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
- **tend's own action is excluded from Dependabot** (`max-sixty/tend` is in the
  github-actions `ignore` list in `.github/dependabot.yaml`). Tend updates flow
  through the nightly `tend/update-workflows` regen (`uvx tend init`), which
  follows structural changes a version-only bump can't — e.g. the 0.1.7 move
  that split the action into `claude/`/`codex/` subdirectories broke the naive
  Dependabot bump #6031. Don't re-add `max-sixty/tend` to Dependabot.
- Automerge: not configured — `pull-request-target.yaml` only validates PR
  titles and handles `pr-backport-web` backports. The automerge job was removed
  in #5753, so bot PRs must be merged manually by a maintainer (or via repo
  branch-protection auto-merge if a maintainer enables it on the PR).

## CI polling during the Dependabot batch

Dependabot opens its whole batch over a couple of minutes (the 2026-08-03 batch
ran 17:14:32 → 17:16:45; across 2026-06 to 2026-08 every batch has landed in
17:12–17:19 UTC), so five or six `tests` matrices compete for runners at once.
The surviving `tests` run on each PR then sits in `QUEUED` for a long time
before it starts — run `30835855220` on #6130 took 73 minutes end to end
(17:14:37 → 18:27:46), far past the 9-minute cap on the poll loop in **CI
Monitoring** in `running-in-ci`.

**Stop after one poll round when every pending check is `QUEUED`.** A `QUEUED`
check has not been allocated a runner, so another round changes nothing: post
the verdict, name the unverified checks, and end. If any pending check is
`IN_PROGRESS`, keep polling — that work is advancing and may still settle.

The `pending()` helper in **CI Monitoring** returns a count without the states,
so it can't tell those two cases apart. Project the states alongside it:

```sh
gh pr view <n> --json statusCheckRollup \
  --jq '[.statusCheckRollup[] | {name: (.name // .context), status: (.status // .state)}]'
```

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
