# t2400 worktree add bare + remote (2026-05-19)

## Goal
Pass test 68: `worktree add <path> <remote/branch>` when the main repo is bare with no HEAD.

## Root causes
1. **Branch delete** — bare main worktrees were treated as occupying `refs/heads/*` via common-dir HEAD, blocking `branch -D main`.
2. **Remote branch add** — explicit `remote/branch` always created a local branch; Git detaches when `can_use_local_refs` is false (orphaned HEAD, no local branches).

## Changes
- `grit/src/commands/worktree_refs.rs`: skip `collect_from_admin` for bare main repos.
- `grit/src/commands/worktree.rs`: detach at remote commit when local refs are unusable.

## Result
`tests/t2400-worktree-add.sh` — **232/232** pass.
