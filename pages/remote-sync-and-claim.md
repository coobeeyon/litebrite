# Remote sync and claim behavior

Litebrite stores tracker data on the orphan `litebrite` branch and uses `origin/litebrite` only when a remote is configured. Local commands read and write the local branch with git plumbing.

`lb sync` is responsible for publishing a missing remote branch. If fetch cannot find `origin/litebrite`, sync pushes the local `litebrite` branch to create it.

`lb claim` and `lb unclaim` call `sync_from_remote()` before editing so they can observe existing remote claims. When origin exists and fetch succeeds, the local branch is fast-forwarded if it is behind. When fetch reports that the remote ref is missing, the command continues and later pushes after the claim/unclaim commit; this creates the remote branch without requiring a separate `lb sync`.

Fetch errors other than a missing remote ref remain hard failures because they may indicate network/auth problems where proceeding would give a false sense that atomic claim coordination happened.

Implemented in main commit `de61ef7` (`Allow claim to publish missing litebrite branch`). The fix was installed locally with `cargo install --path .` and pushed to `origin/main` on 2026-04-29. The wiki notes were pushed on the `trapperkeeper` branch.
