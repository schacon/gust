# wildmatch invalid double-star slash handling

## Goal

Make gitignore pathname wildmatch treat invalid `**` runs as ordinary `*` patterns.

## Notes

- Reproduced `t0008-ignores.sh` failing at `** not confused by matching leading prefix`.
- Pattern `foo**/bar` should match `foo/bar`, but not `foobar`.
- In pathname mode, invalid `**` does not get special recursive-directory semantics; it behaves like ordinary stars.
- The matcher already recognized invalid `**`, but its `*` followed by `/` fast path advanced to the slash in the pattern instead of past it.

## Validation

- `cargo fmt`
- `cargo check -p grit-cli`
- `cargo build --release -p grit-cli`
- `./scripts/run-tests.sh --output-csv /tmp/grit-t0008-fixed.csv --no-catalog t0008-ignores.sh` -> `398/398`
- `cargo fmt --check`
- `cargo test -p grit-lib --lib` -> `199 passed`
- `cargo clippy --fix --allow-dirty -p grit-cli` completed with pre-existing warnings
