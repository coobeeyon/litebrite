Claim a litebrite item.

Usage: /lb-claim <id>

Run `lb claim <id>` to claim the item (fetches from remote, sets claimed_by, pushes).
First push wins — if someone else already claimed it, the command will fail.
Then run `lb show <id>` to confirm.
