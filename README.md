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

## Getting Started

```bash
cargo build --release
./target/release/forja --help
```

Requires `git` >= 2.23 on `PATH`. `gh` is optional (only needed once GitHub
flows land in a later phase).

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

```bash
forja sync
forja cleanup --dry-run
forja setup --config ~/.dotfiles/forja.toml
```

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
