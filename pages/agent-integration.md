# Agent integration

The repo intentionally checks in Codex setup for both Litebrite and Trapperkeeper.

`.codex/config.toml` enables Codex hooks. `.codex/hooks.json` runs `lb prime` and `trk prime` on startup, resume, and clear so agents receive active tracker state plus the persistent wiki protocol before exploring. `.codex/rules/default.rules` allows both `lb` and `trk` commands.

`lb setup codex` also installs the bundled `brite-architect` Codex skill into `$CODEX_HOME/skills/brite-architect`, falling back to `$HOME/.codex/skills/brite-architect` when `CODEX_HOME` is unset. The installer writes the skill when that directory is missing, upgrades the first bundled version that only planned work, and leaves custom skill files untouched. The current skill explicitly tells Codex to create/update brites with `lb create` and `lb dep add` unless the user asks for a draft only.

`.trapperkeeper.json` identifies this repo as using Trapperkeeper's orphan-branch mode. `.trapper_keeper/` is a gitignored worktree backed by the `trapperkeeper` branch, so wiki updates can be committed and pushed independently from source changes on `main`.

`.gitattributes` records the union merge behavior expected for `.trapper_keeper/log.md` updates.

The intended refresh commands are `lb setup codex` and `trk setup codex`; both are idempotent and merge with existing Codex config instead of replacing unrelated hooks.
