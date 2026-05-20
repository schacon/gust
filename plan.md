# PLAN.md — Grit v1 Library Release

**Updated:** 2026-05-18

## Goal

Ship **`grit-lib`** as a fast, solid Git-compatible client engine that covers nearly all
**commonly used, non-interactive** local and network workflows. **`grit-cli`** is a thin
wrapper used to run the upstream harness (`./scripts/run-tests.sh`); **behavior and
tests should be driven through library APIs**, not ad hoc logic in the binary.

### Success criteria

1. **Library-first:** New work lands in `grit-lib/` with small, typed public surfaces;
   `grit/src/commands/` only parses argv, opens `Repository`, maps errors to exit codes.
2. **Harness-backed:** Each milestone below names primary upstream test files to turn
   green (in `./tests`, not `git/t/`). Full upstream parity is not required for v1.
3. **Explicit non-goals for v1** (do not schedule work here):
   - Interactive UX: `rebase -i`, `add -p`, `checkout -p`, `restore -p`, `clean -i`,
     `commit -p`, `am --interactive`, etc.
   - Complex HTTP authentication: Digest, NTLM, Negotiate/SPNEGO, OAuth device flows,
     TLS client certificates, curl-grade multistage auth (Basic + credential helpers
     already in scope is enough).
   - **fsmonitor** / builtin fsmonitor daemon integration and status acceleration
     that depends on it (`core.fsmonitor`, untracked-cache *for fsmonitor paths*).
   - Shell completion, `git send-email`, `git shell`, Scalar, server/daemon parity.
4. **Submodules:** last milestone before v1 tag—after worktrees, sparse, promisor,
   signing, and hooks are solid.

### How to work

1. Claim a task: change `[ ]` → `[~]` and add a line in `logs/YYYY-MM-DD-<topic>.md`.
2. Implement in **`grit-lib`** first; wire CLI only when a command needs it.
3. Validate: `cargo test -p grit-lib --lib`, then `./scripts/run-tests.sh <file>.sh`.
4. Update **`progress.md`** when checkboxes change (counts from this file).
5. Mark `[x]` when the listed harness files for that task are fully passing.

---

## Architecture: target library layout

Consolidate command-sized logic currently living in `grit/src/` into cohesive
`grit-lib` modules. Suggested end state (modules may be merged or split as needed):

| Module / area | Responsibility |
|---------------|----------------|
| `repo`, `odb`, `objects`, `pack` | Open repo, object store, pack read/write |
| `refs`, `reflog`, `reftable`, `state` | Refs, HEAD, bisect state, worktree refs |
| `index`, `split_index`, `sparse_checkout` | Index v2/v3, sparse patterns, checkout scope |
| `worktree` (**new**) | Linked worktrees: add/list/remove/lock/repair/prune |
| `promisor`, `shallow`, `fetch_negotiator` | Partial clone, lazy fetch, backfill |
| `merge_*`, `diff`, `combined_*` | Merge, diff, conflict markers |
| `hooks` | Multihook config + traditional `.git/hooks` execution |
| `signing` (**new**) | Create/verify GPG and SSH-signed commits/tags |
| `transport` (**new**) | upload-pack / receive-pack over bidirectional streams |
| `submodule_*` | Gitlinks, `.gitmodules`, recursive fetch (last) |

Public API style: **`Repository`** (or `GitDir` + work tree) as the entry handle;
inject **time** and **environment** at boundaries; no `.unwrap()` in library code.

---

## Phase 0 — Foundation and CLI thinning (ongoing)

Keep existing strengths stable while moving logic down from the binary.

- [~] **0.1 Repository session API** — Single `Repository::open` path with explicit
  `GitDir`, common dir, work tree, commondir, and config load order documented.
  - Harness: `t1510-repo-setup`, `t1517-outside-repo` (subset: open/discovery only)
- [ ] **0.2 Move transport negotiation to lib** — `fetch_transport` / smart HTTP
  session types live in `grit-lib::transport`; CLI calls `Repository::fetch_pack` /
  `push_pack` style methods.
  - Harness: `t5551-http-fetch-smart`, `t5541-http-push-smart` (regression guard)
- [ ] **0.3 Audit binary-only helpers** — List `grit/src/` modules that implement
  domain rules (index, merge, checkout, signing) and file issues per module to relocate.

---

## Phase 1 — Worktrees (highest priority)

Git-linked checkouts are required for real multi-branch workflows and many porcelain
tests. Most logic belongs in a new **`grit-lib/src/worktree.rs`** (and friends).

### 1.1 Core worktree filesystem model

- [~] Create `worktrees/` registry under common git dir (`worktrees/<id>/gitdir`,
  `commondir`, `locked`, `prunable`, `HEAD`, private refs).
- [~] Resolve `git_dir` vs `common_dir` vs per-worktree refs (`refs/worktree/*`).
- [ ] `config.worktree` overlay and `extensions.worktreeConfig` behavior.

### 1.2 Worktree lifecycle API

- [ ] `Worktree::add(branch, path, opts)` — checkout new or existing branch, write
  `.git` / gitfile, seed `HEAD`, index, optionally fetch.
- [ ] `Worktree::list` / `remove` / `lock` / `unlock` / `move` / `repair` / `prune`.
- [ ] Prevent unsafe operations (checkout branch checked out elsewhere, orphan paths).

### 1.3 Commands using worktree index and refs

- [ ] Index path: per-worktree `index` and shared vs private ref reads on
  `status`, `diff`, `commit`, `reset`, `merge`, `checkout` (non-interactive).
- [ ] Hook env: correct `GIT_WORK_TREE`, `GIT_DIR`, `GIT_COMMON_DIR` per worktree.

### 1.4 Harness targets (worktrees)

- [ ] `t2400-worktree-add`
- [x] `t2402-worktree-list`
- [ ] `t2401-worktree-prune`, `t2406-worktree-repair`
- [ ] `t2404-worktree-config`, `t2407-worktree-heads`
- [ ] `t3908-stash-in-worktree`, `t2205-add-worktree-config` (non-interactive paths)
- [ ] `t1415-worktree-refs`, `t1407-worktree-ref-store` (plumbing)

---

## Phase 2 — Sparse checkout (cone + compatibility)

Build on existing `sparse_checkout.rs` and index `sdir` extension support.

### 2.1 Sparse index and read-tree integration

- [ ] Cone/non-cone pattern load/save (`$GIT_DIR/info/sparse-checkout`,
  `index.sparse` config).
- [ ] `read-tree` / checkout: only materialize included paths; skip-worktree +
  sparse directory entries in index v4.
- [ ] `ls-files`, `add`, `rm`, `mv`, `clean`, `status` respect sparse specification
  (no interactive modes).

### 2.2 Merge and diff under sparse

- [ ] `merge-recursive` / `merge-ort` path limiting vs sparse patterns.
- [ ] `diff` / `diff-tree` skip out-of-cone paths unless `--sparse` / config says otherwise.

### 2.3 Harness targets (sparse)

- [ ] `t1091-sparse-checkout-builtin`
- [ ] `t1092-sparse-checkout-compatibility`
- [ ] `t1011-read-tree-sparse-checkout`, `t1090-sparse-checkout-scope`
- [ ] `t6428-merge-conflicts-sparse`, `t6435-merge-sparse`
- [ ] `t3705-add-sparse-checkout`, `t3602-rm-sparse-checkout`, `t7002-mv-sparse-checkout`

---

## Phase 3 — Partial clone, promisor remotes, lazy objects

Extend `promisor.rs`, `shallow.rs`, and ODB miss handling.

### 3.1 Promisor ODB and packs

- [ ] Align promisor marker with Git (`promisor` remote config, `.promisor` pack sidecars,
  `extensions.partialClone` / `core.promisorRemote`).
- [ ] `odb` miss → promisor remote fetch (single blob/tree/commit) without full clone.
- [ ] `rev-list --missing`, `--exclude-promisor-objects`, connectivity checks.

### 3.2 Clone/fetch filters

- [ ] `filter=blob:none|tree:0|...` on clone/fetch; record filter in config.
- [ ] Lazy fetch in merge, checkout, `cat-file`, `apply` (blob touch paths).
- [ ] `backfill` / `promisor hydrate` as library API.

### 3.3 Shallow + partial interaction

- [ ] Shallow boundary + promisor: fetch deepen, push shallow (non-interactive).

### 3.4 Harness targets (partial / promisor)

- [ ] `t0410-partial-clone`, `t5616-partial-clone`
- [ ] `t6421-merge-partial-clone`, `t1022-read-tree-partial-clone`
- [ ] `t5620-backfill`, `t6110-rev-list-sparse` (promisor-related cases)
- [ ] `t4067-diff-partial-clone`, `t5537-fetch-shallow` (non-interactive)

---

## Phase 4 — Signing (GPG + SSH)

New **`grit-lib/src/signing.rs`** (and small `gpg.rs` / `ssh_sign.rs` if needed).

### 4.1 Commit signing

- [ ] Read `user.signingkey`, `gpg.program`, `gpg.format` (`openpgp` vs `ssh`).
- [ ] Produce `gpgsig` / `gpgsig-sha256` on commit; SSH cert/key parsing per Git 2.x.
- [ ] `commit -S` / `--no-gpg-sign` plumbing via library `CommitOptions`.

### 4.2 Tag signing

- [ ] Annotated tag signing; `verify-tag` / `verify-commit` in library.
- [ ] `push` signed-tag checks where server advertises `allowSignedPush` (client side).

### 4.3 Harness targets (signing)

- [ ] `t7510-signed-commit`
- [ ] `t7528-signed-commit-ssh`
- [ ] `t7031-verify-tag-signed-ssh`
- [ ] `t5534-push-signed` (client-side generation/verification only)

---

## Phase 5 — Hooks (multihook + porcelain integration)

Extend existing **`grit-lib/src/hooks.rs`**.

### 5.1 Hook runner completeness

- [ ] `hook.<name>.*` multihook ordering, `hookPath`, `core.hooksPath`, `init.templateDir`.
- [ ] All v1-required hook names wired with correct stdin/stdout/env:
  `pre-commit`, `prepare-commit-msg`, `commit-msg`, `post-commit`,
  `pre-merge-commit`, `pre-push`, `post-merge`, `post-checkout`, `post-rewrite`,
  `update` / `pre-receive` (push path, client-side invocation of local hooks only).
- [ ] `GIT_HOOK_INFO` for post-checkout; exit code propagation and `--no-verify`.

### 5.2 Library call sites

- [ ] `commit`, `merge`, `checkout`, `push`, `fetch` (where Git runs hooks) call
  `hooks::run_*` with shared `CommitHookEnv`.
- [ ] `git hook run` / `git hook list` delegate to library.

### 5.3 Harness targets (hooks)

- [ ] `t1800-hook`
- [ ] `t7503-pre-commit-and-pre-merge-commit-hooks`
- [ ] `t7504-commit-msg-hook`, `t7505-prepare-commit-msg-hook`
- [ ] `t5571-pre-push-hook`, `t5402-post-merge-hook`, `t5403-post-checkout-hook`
- [ ] `t5407-post-rewrite-hook`, `t5401-update-hooks`

---

## Phase 6 — Core client workflows (non-interactive polish)

Fill gaps in everyday commands **without** interactive modes. Prefer library APIs
used by multiple commands.

### 6.1 Index + worktree + checkout

- [ ] Non-interactive `checkout`, `restore`, `reset` (--hard/mixed/soft, pathspecs,
  merge abort, unborn branch).
- [ ] Racy git / untracked overwrite rules (`t2021`, `t2500`).

### 6.2 Merge and sequencer (non-interactive only)

- [ ] Porcelain `merge`, `pull` (no interactive conflict resolution UI).
- [ ] `cherry-pick`, `revert` (no `rebase -i`); `rerere` optional if cheap.
- [ ] Harness: `t7600-merge`, `t7110-reset-merge`, `t3500-cherry`–`t3511` (skip `-i` cases).

### 6.3 Status, diff, log (no fsmonitor)

- [ ] `status` porcelain v1/v2, branch ahead/behind, conflict summaries.
- [ ] `diff` / `log` formats used by scripts (`--name-status`, `--oneline`, `-L` line-log
  if already started in `line_log.rs`).
- [ ] Harness: `t7508-status`, `t7064-wtstatus-pv2`, `t4202-log` (non-graph-interactive).

### 6.4 Transport hardening (simple auth only)

- [ ] `core.sshCommand` in lib SSH spawn; live fetch/push stream parity.
- [ ] HTTP: keep Basic + credential helper + proactive/empty auth; no new curl auth.
- [ ] Harness: `t5813-proto-disable-ssh`, `t5563-simple-http-auth` (Basic/Bearer helper
  cases only), `t5545-push-options`, `t5547-push-quarantine`.

### 6.5 Maintenance

- [ ] `gc`, `repack`, `maintenance run` as library schedulers over `odb`/`refs`.
- [ ] Harness: `t6500-gc`, `t7700-repack`, `t7900-maintenance` (non-interactive).

---

## Phase 7 — Submodules (last)

Only after Phases 1–6 are stable.

### 7.1 Submodule library API

- [ ] `.gitmodules` parse/cache (`submodule_config*`), gitdir/gitfile layout.
- [ ] `submodule init/update/sync/deinit` without interactive prompts.
- [ ] Recursive fetch/clone; superproject pointer updates on `commit` / `status`.

### 7.2 Cross-command behavior

- [ ] `diff`/`status`/`ls-files` recurse-submodules (non-interactive).
- [ ] Merge/cherry-pick with gitlinks (no `rebase -i` with submodules).

### 7.3 Harness targets (submodules)

- [ ] `t7406-submodule-update`, `t7403-submodule-sync`, `t7407-submodule-foreach`
- [ ] `t7506-status-submodule`, `t2013-checkout-submodule` (non-interactive)
- [ ] `t6437-submodule-merge`, `t4059-diff-submodule-not-initialized`
- [ ] Keep `t7400-submodule-basic` green as regression guard

---

## Phase 8 — v1 release gate

- [ ] **Public API review** — Document stable `grit-lib` entry points in crate rustdoc;
  hide or `#[doc(hidden)]` experimental modules.
- [ ] **Performance** — Benchmark pack indexing, status, diff on large trees (no fsmonitor);
  fix hot paths found in profiling.
- [ ] **CI contract** — `cargo fmt`, `clippy`, `cargo test -p grit-lib --lib`, and
  in-scope harness subset (Phases 1–7 file list) required green for release tag.
- [ ] **Explicit v1 exclusions doc** — Short `docs/v1-scope.md` listing out-of-scope
  interactive, auth, and fsmonitor features so users know what to expect.

---

## Task checklist (execution order)

Use this as the single queue for the next major step. Phases are sequential; tasks
within a phase can be parallelized where noted.

| ID | Phase | Task | Primary harness |
|----|-------|------|-----------------|
| 0.1 | 0 | Repository session API | `t1510-repo-setup` |
| 0.2 | 0 | Transport in `grit-lib` | `t5551`, `t5541` |
| 0.3 | 0 | Binary audit / relocation list | — |
| 1.1 | 1 | Worktree filesystem model | `t1407`, `t1415` |
| 1.2 | 1 | Worktree lifecycle API | `t2400`, `t2402` |
| 1.3 | 1 | Per-worktree index/refs in commands | `t2404`, `t2407` |
| 1.4 | 1 | Worktree repair/prune/lock | `t2401`, `t2406` |
| 2.1 | 2 | Sparse index + checkout | `t1091`, `t1011` |
| 2.2 | 2 | Sparse merge/diff | `t1092`, `t6435` |
| 2.3 | 2 | Sparse add/rm/mv/status | `t3705`, `t3602` |
| 3.1 | 3 | Promisor ODB + packs | `t0410` |
| 3.2 | 3 | Filter clone/fetch + lazy fetch | `t5616` |
| 3.3 | 3 | Backfill + merge on partial | `t5620`, `t6421` |
| 4.1 | 4 | GPG/SSH commit signing | `t7510`, `t7528` |
| 4.2 | 4 | Tag verify + push signed | `t7031`, `t5534` |
| 5.1 | 5 | Multihook runner complete | `t1800` |
| 5.2 | 5 | Hook integration in commit/merge/push | `t7503`–`t7505`, `t5571` |
| 5.3 | 5 | post-checkout/merge/rewrite hooks | `t5402`, `t5403`, `t5407` |
| 6.1 | 6 | Checkout/restore/reset polish | `t7201`, `t7102` |
| 6.2 | 6 | Merge/pull/cherry-pick (no `-i`) | `t7600`, `t3500` |
| 6.3 | 6 | Status/log/diff polish | `t7508`, `t4202` |
| 6.4 | 6 | SSH `core.sshCommand` + push options | `t5813`, `t5545` |
| 6.5 | 6 | gc/repack/maintenance | `t6500`, `t7900` |
| 7.1 | 7 | Submodule init/update API | `t7406` |
| 7.2 | 7 | Submodule status/diff/fetch | `t7506`, `t7400` |
| 8.1 | 8 | API + docs + release gate | — |

---

## Out of scope (do not block v1)

- Interactive patch modes (`-p`, `-i`) for add/checkout/restore/clean/commit/rebase/am
- `rebase -i`, `rebase --exec` todo editor flows
- Digest/NTLM/Kerberos HTTP, TLS client certs, OAuth refresh in credential protocol
- fsmonitor, `core.fsmonitor`, builtin-fsmonitor, fsmonitor-driven fast status
- `git send-email`, tab completion, `git shell`, Scalar monorepo wizard
- Full reftable-by-default, reftable migration UX
- Server-side: `git daemon`, `http-backend`, `receive-pack` on server (client hooks only)

---

## Tracking

- **Progress counts:** derive from checkbox lines in this file (`- [x]` / `[~]` / `[ ]`).
- **Harness dashboard:** `data/test-files.csv` after `./scripts/run-tests.sh`.
- **Session logs:** `logs/` per AGENTS.md loop contract.

When a phase completes, run the listed harness files together and refresh the dashboard
before starting the next phase.
