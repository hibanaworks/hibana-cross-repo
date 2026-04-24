# Cross-Repo Smoke

This repository is the dedicated cross-repo integration harness for the split
`hibana` family.

Goals:

- consume `hibana`, `hibana-epf`, and `hibana-mgmt` through direct GitHub
  dependencies in the default manifest lane
- keep the sibling BOM fixed in manifest `rev` pins, with `Cargo.lock`
  recording the resolved checkout
- keep cross-repo composition tests out of `hibana` core's public-surface guard
  suite
- keep a separate local-worktree smoke lane available through explicit CLI patch
  overlays only

Canonical verification:

- `cargo test --locked` validates the manifest-default exact GitHub BOM and
  the lockfile resolved from it
- `./run_workspace_smoke.sh` remains available when callers want an explicit
  local sibling worktree overlay; it also runs `hibana-epf` and `hibana-mgmt`
  no-default-features checks and std tests against the same sibling set.
  Setting the workspace-smoke env var by hand does not enable local source
  reads
- `./pin_workspace_revs.sh` is the release lane for advancing the locked
  GitHub BOM. It refuses dirty sibling checkouts, writes immutable rev pins
  from published sibling `HEAD`s, refreshes `Cargo.lock`, and reruns the
  locked lane.

The dedicated harness therefore carries two lanes:

1. locked GitHub smoke: manifest `git+rev` dependencies resolved through
   `Cargo.lock`
2. local sibling smoke: current `hibana`, `hibana-epf`, and `hibana-mgmt`
   worktrees plus downstream full validation via `run_workspace_smoke.sh`
3. release rev pinning: clean published sibling commits promoted into the
   locked GitHub BOM via `pin_workspace_revs.sh`
