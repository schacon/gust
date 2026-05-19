# t1415-worktree-refs (10/10)

## Changes

- `grit-lib/src/worktree_ref.rs`: `parse_worktree_ref`, `resolve_ref_storage`, DWIM rules.
- `grit-lib/src/refs.rs`: resolve/read/write/list refs via per-worktree storage paths.
- `grit-lib/src/rev_parse.rs`: `expand_ref` DWIM + ambiguous refname warnings.
- `grit/src/commands/for_each_ref.rs`: list refs from common + linked admin with filtering.

## Behavior

- `refs/worktree/*`, `refs/bisect/*`, `refs/rewritten/*` stored per linked worktree admin dir.
- `main-worktree/HEAD` and `worktrees/<id>/HEAD` resolve through common dir layout.
- `worktree/foo` DWIM expands to `refs/worktree/foo` via `refs/{0}` rule.
