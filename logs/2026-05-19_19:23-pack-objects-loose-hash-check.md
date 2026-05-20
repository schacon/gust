# pack-objects loose hash validation

## Goal

Make `pack-objects` reject loose object files whose inflated contents hash to a different object ID than their path.

## Notes

- Reproduced `t1060-object-corruption.sh` failing at `clone --no-local --bare detects misnamed object`.
- The source repo replaced a reachable blob's loose object file with different valid object contents.
- `pack-objects` was reading the object by path without verifying the payload hash, so transport clone succeeded.
- Fixed `pack-objects` object loading to verify local loose objects before falling through to packed objects or lazy fetch.
- Kept `blob:none` partial clone working by dropping blobs from the pack list before strict object reads.

## Validation

- `cargo fmt`
- `cargo check -p grit-cli`
- `cargo build --release -p grit-cli`
- `./scripts/run-tests.sh --output-csv /tmp/grit-t1060-fixed.csv --no-catalog t1060-object-corruption.sh` -> `16/17`
- `cargo fmt --check`
- `cargo test -p grit-lib --lib` -> `199 passed`
- `cargo clippy --fix --allow-dirty -p grit-cli` completed with pre-existing warnings
