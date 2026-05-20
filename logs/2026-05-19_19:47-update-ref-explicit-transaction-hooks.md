# update-ref explicit transaction hook phases

## Goal

Avoid duplicate `reference-transaction` hook phases when `git update-ref --stdin` uses explicit `prepare` and `commit` commands.

## Notes

- Reproduced `t1416-ref-transaction-hooks.sh` failing at `hook gets all queued symref updates`.
- The explicit `commit` command prepared the transaction, then called `commit_batch_staged`, which prepared and committed hooks again.
- Split batch verification/application from hook orchestration so implicit batches keep their hook behavior while explicit transactions run each phase once.

## Validation

- `cargo fmt`
- `cargo check -p grit-cli`
- `cargo build --release -p grit-cli`
- `./scripts/run-tests.sh --output-csv /tmp/grit-t1416-fixed.csv --no-catalog t1416-ref-transaction-hooks.sh` -> `10/10`
- `cargo fmt --check`
- `cargo test -p grit-lib --lib` -> `199 passed`
- `cargo clippy --fix --allow-dirty -p grit-cli` completed with pre-existing warnings
