# t2205 / t3908 — ls-files worktree traversal

## t3908-stash-in-worktree

Already passing (2/2); no code changes.

## t2205-add-worktree-config (13/13)

### Fixes

1. **`path_for_disk_compare`** in `grit-lib/src/git_path.rs` — macOS `/private` aliasing for `abspath_part_inside_repo`.
2. **`ls-files` absolute pathspecs** — use `abspath_part_inside_repo` when `core.worktree` differs from `.git` location.
3. **Own `.git` directory** — do not treat the repository's `.git` as a nested repo; recurse for untracked/ignored walks.
4. **`--directory` + own `.git`** — emit a single directory marker (`repo/` or `./` via pathdiff) for plain `--others`; still recurse for `--ignored`.
5. **`collapse_to_directories`** — keep direct files (`../file-tracked`); collapse only nested subdirectories to `dir/`.
