# t1407-worktree-ref-store (4/4)

## Changes

- `grit/src/main.rs`: delegate `test-tool ref-store worktree:*` to `test_tool_ref_store::run`.
- `grit/src/commands/test_tool_ref_store.rs`: use `worktree_ref::resolve_ref_storage` for resolve/create paths.
