# core.logAllRefUpdates reflog creation

## Goal

Make reflog creation honor `core.logAllRefUpdates=0` so unreachable commits are not kept alive by reflogs that Git would not create.

## Notes

- Reproduced `t1420-lost-found.sh` failing because `fsck --lost-found` only wrote the dangling blob, not the reset-away commit.
- The reset-away commit was still reachable through `.git/logs/HEAD`, created despite `core.logAllRefUpdates = 0`.
- Fixed `append_reflog` so nonempty messages do not force creation of a missing reflog. Existing reflogs still append, and explicit `force_create` still creates.

## Validation

- `cargo fmt`
- `cargo check -p grit-cli`
- `cargo build --release -p grit-cli`
- `./scripts/run-tests.sh --output-csv /tmp/grit-t1420-fixed.csv --no-catalog t1420-lost-found.sh` -> `2/2`
- `cargo fmt --check`
- `cargo test -p grit-lib --lib`
- `cargo clippy --fix --allow-dirty -p grit-lib` (completed with pre-existing warnings)
