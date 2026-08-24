<p align="center">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/git-F05032?style=for-the-badge&logo=git&logoColor=white" alt="git"/>
  <img src="https://img.shields.io/badge/GitHub_CLI-181717?style=for-the-badge&logo=github&logoColor=white" alt="GitHub CLI"/>
</p>

# forja

**Collapse repetitive git/GitHub sequences into single, safe commands.**

A Rust CLI for developers who know what `git rebase` does and just don't
want to type the same six-command sequence for the hundredth time.

---

## Key Features

- **Flows** — `sync` and `cleanup` replace multi-step git sequences with one
  command, with safety checks a hand-written alias never has.
- **Compliance** — a declarative `forja.toml` describes how git should be
  configured; `setup` applies it idempotently.
- **Safe by default** — never force-pushes, never auto-stashes, never
  deletes an unmerged branch, always aborts before touching anything if a
  precondition isn't met.
- **Scriptable** — non-interactive by default, `--dry-run` on every command,
  structured `--json` output, meaningful exit codes.

---

## Installation

```bash
cargo install --git https://github.com/nadezhdkov/forja forja
```

Builds `forja` from source and puts it on your `PATH` via `~/.cargo/bin`.
Requires the [Rust toolchain](https://rustup.rs) and `git` >= 2.23 on
`PATH`. `gh` is optional (only needed once GitHub flows land in a later
phase).

Prebuilt binaries (no Rust toolchain needed) are planned via `cargo-dist`
but not published yet — for now, building from source is the way to install.

### Building from source (for local development)

```bash
git clone https://github.com/nadezhdkov/forja.git
cd forja
cargo build --release
./target/release/forja --help
```

---

## Commands

| Command | What it does |
|---|---|
| `forja init` | Generate a commented `forja.toml` scaffold |
| `forja show` | Display the loaded, normalized config (read-only) |
| `forja doctor` | Check that `git`/`gh` are installed and ready |
| `forja setup` | Apply `[git]` config via `git config --global`, idempotently |
| `forja sync` | Fetch, rebase (or merge) onto the base branch, and push |
| `forja cleanup` | Delete local branches already merged and removed on the remote |

All commands accept `--dry-run` (show the plan, change nothing), `--yes`
(skip confirmations), and `--json` (structured output).

---

## Usage

### `forja doctor` — check your environment

Run this first on any machine. It never changes anything.

```
$ forja doctor
  ✓ git: 2.43.0
  ✓ gh: gh version 2.40.1 (2023-12-13)
  ✓ gh auth: authenticated
```

Missing `gh` (or being logged out) is only a warning in the current MVP —
`forja` doesn't need GitHub for `sync`/`cleanup`. A missing or too-old `git`
is the only thing that fails the check (exit `3`).

### `forja init` — scaffold a config

```
$ forja init
wrote ./forja.toml
```

Generates a commented `forja.toml` with every field but `version` commented
out — `forja` never invents your name or email. Uncomment and fill in what
you want; everything is optional.

### `forja show` — see what forja actually loaded

Read-only — never invokes `git`, never mutates anything.

```
$ forja show
version: 1

[git]
  user_name      = Ada Lovelace
  user_email     = ada@example.com
  default_branch = main

  [git.aliases]
    st = "status -sb"

[flow]
  strategy           = rebase
  auto_push          = true
  base_branch        = (detected from remote)
  protected_branches = ["main", "master"]
```

Useful for confirming defaults are what you expect, or piping `--json` into
another tool.

### `forja setup` — apply your `[git]` config

Preview first, then apply:

```
$ forja --dry-run setup
  user.name : (unset) -> Ada Lovelace
  user.email : (unset) -> ada@example.com
  alias.st : (unset) -> status -sb

$ forja setup
  user.name : (unset) -> Ada Lovelace
  user.email : (unset) -> ada@example.com
  alias.st : (unset) -> status -sb
applied 3 of 3 change(s)

$ forja setup
git config already matches forja.toml — no changes needed
```

Only fields that actually diverge get written — running it again reports
zero changes. Your existing `~/.gitconfig` is backed up to
`~/.gitconfig.forja.bak` before the first write.

### `forja sync` — fetch, integrate, push, in one safe step

```
$ forja sync
Current branch: feature/login
Base:           origin/main

  ✓ working tree is clean
  ✓ branch is not protected
  → git fetch origin
  → git rebase origin/main
  → git push --force-with-lease origin feature/login
feature/login synced with origin/main and pushed.
```

If anything is off, `sync` refuses instead of guessing — and exits before
touching your repository:

```
$ forja sync
error: working tree is dirty (1 file(s) changed)

Aborted before any changes. Commit or stash your changes:
  git stash push -m "wip"
$ echo $?
4
```

The same happens on a protected branch (`main`/`master` by default), when
the base branch can't be determined, or when a rebase hits a real conflict
— `forja` leaves the repo exactly where git left it and lets you resolve it
by hand (`git rebase --continue` / `--abort`).

### `forja cleanup` — delete branches git already knows are done

A branch only qualifies if it's **both** merged into the base branch **and**
confirmed deleted on the remote — never a branch you simply never pushed.

```
$ forja cleanup
branches to delete (merged and removed on the remote):
  - feature/login
  - hotfix/typo
delete these branches? [y/N] y
deleted 2 of 2 branch(es)
```

Skip the prompt in scripts with `--yes`; preview with `--dry-run`. Deletion
always uses `git branch -d`, never `-D` — an unmerged branch is refused by
git itself even if something upstream miscalculates.

---

## Configuration

```toml
version = "1"

[git]
user_name  = "Ada Lovelace"
user_email = "ada@example.com"

[git.aliases]
st = "status -sb"

[flow]
protected_branches = ["main", "develop"]
```

Every field is optional — `sync`/`cleanup` work with zero config. See
[PRD-forja.md](PRD-forja.md) §8 for the full schema.

---

## Testing

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Flow tests (`sync`, `cleanup`) run against real, disposable git
repositories with a local bare "origin" — no network, no GitHub.

---

## Status

MVP complete: all of `init`, `show`, `doctor`, `setup`, `sync`, and
`cleanup` are implemented and tested. See [CLAUDE.md](CLAUDE.md) for
architecture notes and [PRD-forja.md](PRD-forja.md) for the full spec,
roadmap, and design rationale.

## License

MIT OR Apache-2.0
