# Litebrite

A lightweight, git-native issue tracker that lives on an orphan branch in your repo. Heavily inspired by [beads](https://github.com/steveyegge/beads) — the data model (items, deps, parent/child, blocking), CLI design, ID prefix resolution, and Claude Code integration are all derived from beads. Litebrite is a simplified Rust reimplementation.

Stores items (epics, features, tasks) as JSON on an orphan `litebrite` branch — no files in your working tree. Supports atomic claiming (first push wins) and schema-aware three-way merge for concurrent edits.

## Install

```
cargo install --path .
```

## Quick Start

```
lb init                              # create litebrite branch in current git repo
lb create "Set up CI pipeline"       # create a task (default type)
lb create "Auth system" -t epic      # create an epic
lb create "Login page" --parent lb-a3f2  # create with parent
lb list                              # list open items
lb list --tree                       # list as a tree
lb ready                             # show unblocked, unclaimed items by priority
lb claim lb-a3f2                     # claim an item (fetches + pushes)
lb close lb-a3f2                     # close an item (clears claim)
lb sync                              # sync local changes with remote
```

## Commands

| Command | Network? | Description |
|---------|----------|-------------|
| `lb init` | No | Initialize `litebrite` branch (detects existing remote branch) |
| `lb create <title>` | No | Create an item (`-t epic/feature/task`, `-p <priority>`, `--parent <id>`, `-d <desc>`) |
| `lb show <id>` | No | Show item details, deps, children, and claim status |
| `lb list` | No | List items (`--all`, `-t <type>`, `-s <status>`, `--tree`) |
| `lb update <id>` | No | Update fields (`--title`, `--status`, `-t`, `-p`, `-d`, `--parent`) |
| `lb close <id>` | No | Close an item (clears claim if set) |
| `lb delete <id>` | No | Delete an item and its deps |
| `lb dep add <id> --blocks <id>` | No | Add a blocking dependency |
| `lb dep rm <from> <to>` | No | Remove a dependency |
| `lb dep list <id>` | No | List deps for an item |
| `lb ready` | No | Show open + unblocked + unclaimed items sorted by priority |
| `lb claim <id>` | **Yes** | Claim an item (fetch + set claimed_by + push; first push wins) |
| `lb unclaim <id>` | **Yes** | Release a claim (fetch + clear claimed_by + push) |
| `lb sync` | **Yes** | Sync with remote (fetch + three-way merge + push) |
| `lb prime` | No | Output AI-optimized context for Claude Code hooks |
| `lb setup claude` | No | Set up Claude Code integration (hooks + permissions) |

Local-only commands are fast — no network. Use `lb sync` to share changes. `lb claim`/`lb unclaim` always sync because atomicity matters.

## Item Types

- **epic** — large body of work
- **feature** — a distinct capability
- **task** — a unit of work (default)

## Statuses

`open`, `closed`

Blocked is derived from dependencies (an item with unclosed blockers won't appear in `lb ready`). Claimed is a separate `claimed_by` field set by `lb claim` and cleared by `lb close`/`lb unclaim`.

Closed items are hidden from `lb list` by default (use `--all` to show them).

## Claiming

`lb claim <id>` fetches from remote, sets `claimed_by` to your `git config user.name`, and pushes. First push wins — if someone else already claimed the item, the command fails. This gives atomic work assignment without a central server.

`lb unclaim <id>` releases a claim. `lb close <id>` also clears any claim.

## IDs

Items get short IDs like `lb-a3f2`. You can use any unique prefix to reference an item (e.g., `lb-a3` if unambiguous).

## Storage

All data lives in `store.json` on an orphan `litebrite` git branch — nothing in your working tree. Reads use `git show`, writes create commits via git plumbing. When `lb sync` encounters diverged histories, it performs a schema-aware three-way merge: non-conflicting changes to different items or different fields merge cleanly; for `claimed_by` conflicts, the remote version wins (first push won).

`lb init` in a clone of an existing litebrite repo detects the remote branch and sets up tracking automatically.

## Claude Code Integration

Litebrite integrates with [Claude Code](https://claude.com/claude-code) via hooks. Run:

```
lb setup claude
```

This writes `.claude/settings.local.json` with:
- SessionStart and PreCompact hooks that run `lb prime`
- `Bash(lb:*)` permission so Claude can run `lb` commands

The `lb prime` command outputs AI-optimized context (claimed items, ready items, session protocol, CLI reference). It runs automatically at session start and before context compaction, giving Claude persistent awareness of your tracker state. The CLI reference in the prime output is sufficient for Claude to operate all `lb` commands — no slash commands needed.

### Notes

- `lb setup claude` is idempotent — safe to run repeatedly
- It merges into existing `.claude/settings.local.json` without clobbering other config
- `lb prime` exits silently in non-git or non-litebrite directories, so global hooks are safe
- `.claude/settings.local.json` is typically gitignored (per-machine); each developer runs `lb setup claude` after cloning

## License

MIT
