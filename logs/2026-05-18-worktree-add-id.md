# Worktree add: unique admin IDs (2026-05-18)

## Problem

`t2400-worktree-add` test 12 failed when adding `sub/here` after test 8 created
`here`: Grit used basename-only for `.git/worktrees/<id>/`, colliding with Git's
numeric-suffix scheme (`here`, `here1`, …).

## Changes

- `grit-lib::worktree`: `worktree_path_basename`, `sanitize_worktree_id_component`,
  `allocate_worktree_admin_dir` (matches Git `add_worktree` id allocation).
- `grit worktree add` uses `allocate_worktree_admin_dir` instead of rejecting
  duplicate basenames.

## Harness

- Build **debug** `target/debug/grit` before `./scripts/run-tests.sh` (test-lib
  prefers debug over `tests/grit` copy).
- Re-run: `./scripts/run-tests.sh t2400-worktree-add.sh` after enabling in CSV.
