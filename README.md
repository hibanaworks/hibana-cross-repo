# Cross-Repo Smoke

This repository is the dedicated cross-repo integration harness for the split
`hibana` family.

Goals:

- consume `hibana`, `hibana-epf`, and `hibana-mgmt` through their public GitHub
  repositories at immutable revisions
- avoid local path or registry-patch assumptions in the harness itself
- keep cross-repo composition tests out of `hibana` core's public-surface guard
  suite
- keep the split reproducible through immutable dependency pins rather than
  sibling checkout assumptions

Local workspace verification:

- `cargo test` validates the published immutable revision shape
- `./run_workspace_smoke.sh` overlays the sibling worktrees through
  command-line Cargo patches so the harness exercises the current local
  branches without changing the pinned manifest contract

The dedicated harness therefore carries two explicit smoke lanes:

1. release smoke: immutable GitHub revisions only
2. workspace smoke: current local `hibana`, `hibana-epf`, and `hibana-mgmt`
   worktrees via CLI patch overlay
