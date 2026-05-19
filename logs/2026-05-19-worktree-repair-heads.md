# Worktree repair + occupied refs (2026-05-19)

## t2406-worktree-repair (24/24)

- Fixed macOS `path_for_git_storage`: `strip_prefix("/private")` must rejoin with `/` so `exists()` checks `/tmp/...` not `tmp/...`.
- Normalize `common` / `worktrees_dir` in `cmd_repair` for consistent `/tmp` vs `/private/tmp` comparisons.
- Treat non-existent gitfile targets as `.git file broken` (not `.git file incorrect`).

## t2407-worktree-heads (12/12)

- Trim `ref:` target in `worktree_refs::collect_from_admin` (newline broke map lookup).
- Run worktree occupation checks before descendant `list_refs` in `branch -f` (ENOTDIR on `refs/heads/wt-N/`).
- `fetch`: use `worktree_refs::branch_occupied_any_worktree` (bisect/rebase/update-refs).
- `rebase --update-refs`: write `rebase-merge/update-refs`; inject `update-ref` lines in `rebase -i` todo.
