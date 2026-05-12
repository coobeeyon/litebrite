# Log
## [2026-05-12] fixed | codex hooks feature flag migration
Recorded that Litebrite and Trapperkeeper setup now use `[features].hooks = true` and remove the deprecated `[features].codex_hooks` key when refreshing Codex config.

## [2026-05-12] fixed | brite architect execution behavior
Recorded that the `brite-architect` skill must create or update Litebrite items with `lb` commands, and that `lb setup codex` upgrades the first bundled skill version while preserving custom skill directories.

## [2026-05-12] reviewed | codex brite skill setup
Reviewed the `lb-kjes` branch behavior: `lb setup codex` now installs the bundled `brite-architect` skill into Codex home only when absent, while preserving existing skill directories on rerun.

## [2026-05-12] documented | codex and trapperkeeper agent setup
Recorded that the repo intentionally checks in Codex hooks for both `lb prime` and `trk prime`, plus the Trapperkeeper orphan-branch worktree setup files.

## [2026-05-05] updated | recorded install and push completion
Added completion state for the missing remote `litebrite` branch claim fix: commit `de61ef7` was installed locally and pushed to `origin/main`; wiki notes were pushed on `trapperkeeper`.

## [2026-04-28] documented | remote branch bootstrap for claim
Recorded that claim/unclaim treat a missing `origin/litebrite` ref as publishable local state while preserving hard failures for other fetch errors.
