# Litebrite

A lightweight, git-friendly issue tracker that lives in your project directory.

Litebrite stores items (epics, features, tasks) as JSON in a `.litebrite/` directory alongside your code. Items have types, statuses, priorities, and support parent/child and blocking dependency relationships.

## Install

```
cargo install --path .
```

## Quick Start

```
lb init                              # create .litebrite/ in current dir
lb create "Set up CI pipeline"       # create a task (default type)
lb create "Auth system" -t epic      # create an epic
lb create "Login page" --parent lb-a3f2  # create with parent
lb list                              # list open items
lb list --tree                       # list as a tree
lb ready                             # show unblocked items by priority
```

## Commands

| Command | Description |
|---------|-------------|
| `lb init` | Initialize `.litebrite/` in the current directory |
| `lb create <title>` | Create an item (`-t epic/feature/task`, `-p <priority>`, `--parent <id>`) |
| `lb show <id>` | Show item details, deps, and children |
| `lb list` | List items (`--all`, `-t <type>`, `-s <status>`, `--tree`) |
| `lb update <id>` | Update fields (`--title`, `--status`, `-t`, `-p`, `--parent`) |
| `lb close <id>` | Close an item |
| `lb delete <id>` | Delete an item and its deps |
| `lb dep add <id> --blocks <id>` | Add a blocking dependency |
| `lb dep rm <from> <to>` | Remove a dependency |
| `lb dep list <id>` | List deps for an item |
| `lb ready` | Show open + unblocked items sorted by priority |

## Item Types

- **epic** -- large body of work
- **feature** -- a distinct capability
- **task** -- a unit of work (default)

## Statuses

`open`, `in_progress`, `blocked`, `deferred`, `closed`

Closed items are hidden from `lb list` by default (use `--all` to show them).

## IDs

Items get short IDs like `lb-a3f2`. You can use any unique prefix to reference an item (e.g., `lb-a3` if unambiguous).

## Storage

All data lives in `.litebrite/store.json`. Commit it to version control to share project state with your team.

## License

MIT
