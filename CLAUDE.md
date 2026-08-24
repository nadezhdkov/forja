# CLAUDE.md

Guidance for Claude Code (and any other agent) working in this repository. Full
spec lives in [PRD-forja.md](PRD-forja.md) — read it before making product
decisions; this file is the condensed operating manual.

## What this project is

`forja` is a Rust CLI that collapses repetitive git/GitHub command sequences
into single, safe, predictable commands. Two pillars:

1. **Flows** — commands that replace 3+ manual steps crossing the git↔GitHub
   boundary (`sync`, `cleanup`, later `pr`, `repo new`).
2. **Compliance** — a declarative `forja.toml` describing desired git
   configuration, applied idempotently (`setup`, `show`, `doctor`).

Status: **M1 done — the full MVP (PRD §7.1) is implemented.** All seven MVP
items (`init`, `show`, `doctor`, `setup`, `sync`, `cleanup`, `--dry-run`)
work end-to-end with tests. Per the phase-progression rule (§7.3, R-01),
**Phase 2 (`pr`, `repo new`, `gh` integration) should not start** until
`sync`/`cleanup` have had two weeks of real daily use (§18, M1.5) — flag it
if asked to start on that before then.

## Current architecture

Cargo workspace with two crates (`crates/`):

- **`forja-core`** (lib, no `clap` dependency) — the domain layer:
  - `config/` — `RawConfig`/`RawGitConfig`/`RawFlowConfig` (`schema.rs`)
    deserialize every field as `Option<toml::Value>` rather than a typed
    field. This is deliberate: it defers type-checking to the validation
    pass in `mod.rs`, which is what lets RF-02 collect *every* problem in
    one config instead of failing on the first `serde` type mismatch.
    `mod.rs` also normalizes into `ForjaConfig`/`GitConfig`/`FlowConfig`
    with defaults applied, and captures unknown keys into a `warnings: Vec<String>`
    (DD-06) via `#[serde(flatten)]` into a `toml::Table` at every level.
  - `error.rs` — `ForjaError` (thiserror), with `exit_code()` mapping each
    variant to the §9.2 contract. Add new variants here as new failure modes
    show up; extend `exit_code()` in the same commit.
  - `exec.rs` — `CommandRunner` trait + `SystemCommandRunner`, always
    spawning via argument vector (§15). `sync`/`cleanup`/`setup` should all
    go through this rather than calling `std::process::Command` directly, so
    they stay testable against a fake runner.
  - `template.rs` — the static `forja init` scaffold (everything but
    `version` commented out; `init` never invents a name/email).
  - `doctor.rs` — `run_checks(&dyn CommandRunner) -> DoctorReport`. Pure
    domain logic: parses `git --version`/`gh --version`/`gh auth status`
    output and classifies each into `Ok`/`Warning`/`Failed`. `gh` absence or
    no-auth is always `Warning`, never `Failed`, per RF-05 in the MVP.
  - `setup.rs` — `compute_plan`/`apply_plan`. `compute_plan` reads current
    state one `git config --global --get <key>` at a time (DD-04) and only
    includes keys that actually diverge (RF-07). `default_branch` is always
    considered (it's a plain `String` with a default already applied at
    config-load time), while `editor`/`pull_rebase` — which stayed
    `Option<T>` through normalization — are only considered when `Some`,
    which is what keeps "absent optional fields are never written" true.
    `apply_plan` returns an `ApplyOutcome { applied, error }` rather than a
    bare `Result`, so a failure partway through still reports what already
    succeeded (RF-11, DD-02).
  - `sync.rs` — `plan_sync`/`execute_sync`, implementing RF-09 and DD-08.
    `plan_sync` runs only the read-only RF-09 preconditions (repo check,
    clean-tree check, protected-branch check, base-branch detection) and
    never touches the repo; `execute_sync` does the actual
    fetch/rebase-or-merge/push. Base branch is always stored as a **plain
    name** (`"main"`, never `"origin/main"`) — an explicit `flow.base_branch`
    wins outright, otherwise it's read from `origin/HEAD` and the `origin/`
    prefix is stripped, so the rest of the code never has to care which
    source a name came from. `ensure_git_repo`, `current_branch`, and
    `detect_base_branch` are `pub(crate)` because `cleanup.rs` reuses them
    directly rather than duplicating the same three `git` invocations.
    Conflict detection is a string check for `"CONFLICT"` in the
    rebase/merge output — deliberately not `--abort`/`--continue`; DD-08
    requires leaving the repo exactly where git left it.
  - `cleanup.rs` — `plan_cleanup`/`delete_branches`, implementing RF-10.
    A candidate must be **both** merged into the base branch (`git branch
    --merged origin/<base>`) **and** have an upstream `git` confirms is
    gone (`%(upstream:track)` containing `[gone]` in `for-each-ref`) —
    intentionally narrower than "never had an upstream," so a branch the
    user simply never pushed is never touched even if merged. Deletion
    uses `git branch -d` (never `-D`): git's own safe-delete refusal on an
    unmerged branch is a second, independent enforcement of DD-08 beyond
    the `--merged` filter above — that redundancy is deliberate, not
    an oversight to simplify away.
- **`forja`** (bin) — thin CLI: `cli.rs` (clap derive, global flags),
  `commands/<name>.rs` per subcommand, `main.rs` dispatches and turns
  `ForjaError` into `eprintln!` + `exit_code()`. `doctor` is the one
  exception to that dispatch pattern — it returns a `DoctorReport` instead of
  a `Result`, and `main.rs` maps `report.has_failure()` to exit 3 directly,
  since an individual check failing is data, not a control-flow error.
  Adding a subcommand otherwise means: add a `Command` variant in `cli.rs`, a
  `commands/<name>.rs`, one match arm in `main.rs` (RF-07).
- `commands/setup.rs` also owns backing up the global gitconfig before the
  first write of a run (`~/.gitconfig.forja.bak`, §15) — it resolves the
  target file via `GIT_CONFIG_GLOBAL` first, falling back to `~/.gitconfig`,
  so the backup always matches the file `git config --global` is about to
  touch (this is also what makes it safe to test against a temp file per
  §13).
- `commands/cleanup.rs` is the only place that reads stdin (a plain
  `y`/`yes` confirmation, skippable with `--yes`) — nothing else in the CLI
  is interactive.
- **Exit-code split within a single flow (`sync`, established for future
  flows too):** only the RF-09 *verification* steps, failed base-branch
  detection, and a detected rebase/merge conflict use **exit 4** — nothing
  was changed, or `forja` deliberately refused. A `fetch`/`push` that fails
  for an external reason (network, permissions, a `--force-with-lease`
  rejection) is **exit 1** via `ForjaError::CommandFailed` — that's an
  external command failing, not a safety refusal. Keep this split when
  adding new flows: exit 4 is earned by "nothing happened yet" or "git left
  us in a state we won't paper over," not by "a git command returned
  non-zero."

Tests: unit tests live next to the code they cover (`#[cfg(test)] mod tests`,
each with its own small fake `CommandRunner` keyed by `(program, args)` —
see `sync.rs`/`cleanup.rs`/`doctor.rs`/`setup.rs` for the pattern). CLI-level
integration tests live in `crates/forja/tests/` using `assert_cmd`.
`sync`/`cleanup` integration tests use a shared `tests/common/mod.rs`
(`TestRepo`) that builds a real bare "origin" plus a working clone per PRD
§13 — no network, no GitHub, real `git` throughout. That module carries
`#![allow(dead_code)]` because each integration test binary only exercises a
subset of its helpers; that's expected, not a sign of dead code to prune.

## Non-negotiable rules (do not violate without asking the user)

These come from PRD §4.2 (non-goals) and §12 (DD-05, DD-07, DD-08). They are
product-defining constraints, not style preferences:

- **Never wrap single-step git commands.** No `forja commit`, `forja push`,
  `forja status`. A new subcommand is only admissible if it replaces **3+
  manual steps** or embeds a safety check git doesn't do alone (DD-07).
- **Never store credentials.** No token/password field ever goes in
  `forja.toml`, not even optional. Auth is 100% delegated to an
  already-authenticated `gh` (DD-05, N3).
- **Flows abort, they never improvise (DD-08):**
  - Never `git push --force` — use `--force-with-lease` only.
  - Never touch the working tree without consent — no automatic `stash`.
  - Never delete an unmerged branch, even with `--yes`.
  - Never operate on a branch listed in `protected_branches`.
  - Never auto-resolve a rebase conflict — stop, explain, exit 4.
  - When a flow hits ambiguity, it stops and returns control to the user.
- **No telemetry, no auto-install.** `forja` verifies and reports dependency
  state; it never installs `git`, `gh`, or runtimes (N4, N7).
- **Never interactive-prompt in a way that breaks scripting.** Default
  non-interactive; `--yes` skips confirmations (N5).
- **Never invoke external commands via a shell.** Always exec with an
  argument vector — no string interpolation, no injection surface (§15).
- **Never rewrite the user's `forja.toml`** without an explicit `--write`
  flag (§8.5).

## Exit code contract (§9.2)

| Code | Meaning |
|---|---|
| 0 | Success (including `--dry-run`) |
| 1 | External command failed |
| 2 | Config or usage error |
| 3 | Missing/unauthenticated external dependency |
| 4 | Safety precondition failed — flow deliberately aborted |
| 130 | SIGINT |

Exit code 4 is the most important one to get right in tests: it's what
distinguishes "the tool broke" from "the tool refused to do something
dangerous." Every DD-08 rule needs a dedicated test asserting exit 4.

## MVP scope (§7.1) — all seven items now implemented

`init`, `show`, `doctor`, `setup`, `sync`, `cleanup`, and `--dry-run` are all
done. `sync` and `cleanup` are purely local (git only, no `gh`, no network
auth), as required — this must stay true until the MVP is validated with 2
weeks of real use (§7.1, §18 M1.5). Do not start on GitHub integration
(`pr`, `repo new`), profiles, `forja capture`, multi-path config lookup, or
runtime verification until that milestone is met (Regra de progressão,
§7.3) — this is the current gate on all further scope.

## Config schema (`forja.toml`, §8)

- `version` field is mandatory, must be `"1"` for the current schema.
- Unknown keys → warning, not error (DD-06) — lets a config written for a
  future schema still load on an older binary.
- Missing required keys → error, exit 2.
- All config is optional for flows: `forja sync`/`forja cleanup` must work
  with zero config, using sensible defaults.
- Sections: `[git]` (used by `setup`), `[git.aliases]` (free string→string
  table), `[flow]` (used by `sync`/`cleanup`). See PRD §8.2 for exact fields,
  types, and defaults.

## Design decisions to respect (§12)

- **DD-01**: config always overwrites machine state on conflict, no prompt —
  but `--dry-run`/normal output must show `old → new` transitions.
- **DD-02**: no rollback in `setup`; each `git config` write is independent
  and idempotent. On partial failure, list what was already applied (RF-11).
- **DD-03**: always compute the diff before writing, even without
  `--dry-run` — avoids duplicated code paths and unnecessary writes.
- **DD-04**: MVP writes config one `git config --global` process per key;
  don't reach for `gix-config` or batching unless RNF-02 is actually
  violated — direct `git` shelling is safer than reimplementing config
  writing.

## Testing expectations (§13)

- Integration tests for `setup` use `GIT_CONFIG_GLOBAL` pointed at a temp
  file — never touch the real machine gitconfig.
- Integration tests for `sync`/`cleanup` use real temporary repos with a
  local bare repo as "origin" (`git init --bare`) — no network, no GitHub.
- Every DD-08 safety rule needs its own test that builds the dangerous
  scenario and asserts exit 4. These are the highest-priority tests in the
  suite.
- Idempotency test: run `setup` twice, second run reports zero changes.
- Snapshot `--dry-run` output so format changes are visible in review.
- CI: build + `clippy -D warnings` + full suite on Linux and macOS.

## Non-functional constraints (§11)

- No `panic!`/`unwrap()`/`expect()` on any path reachable from user input —
  enforced by lint in CI (RNF-04).
- Every error message states what failed, why, and what to do about it — a
  message with no suggested action is a bug (RNF-05).
- `show`/`doctor` < 50ms; `setup` < 300ms for 20 aliases (RNF-02).
- Portable across Linux/macOS without `cfg(platform)` in domain code
  (RNF-03). Windows is not tested in the MVP.
- Adding a new flow should mean one new module implementing the flow trait
  plus one registration line — no changes to parsing, CLI, or the command
  executor (RNF-07).

## Open questions (§17) — resolved vs. still open

**Resolved during implementation** (don't re-litigate without a reason):
- **QA-02** — `sync` on a protected branch always aborts (exit 4). No
  fast-forward exception; DD-08 stays absolute.
- **QA-03** — Base branch, when `flow.base_branch` isn't set, comes purely
  from `origin/HEAD`. `[git].default_branch` is never consulted as a
  fallback for this.
- **QA-05** — `sync` never confirms, including before push (matches §9.4
  and N5). `cleanup` still confirms before deleting, skippable with `--yes`.

**Still open** — flag instead of silently deciding:
- **QA-01** — the `forja` name itself is provisional (validation checklist
  in PRD §17).
- **QA-04** — whether `setup`'s `git config` scope should ever extend to
  `--local`. MVP is `--global` only.
