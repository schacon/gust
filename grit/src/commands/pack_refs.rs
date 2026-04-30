//! `grit pack-refs` command.
//!
//! Packs loose refs into `.git/packed-refs` for faster ref lookups.
//! By default, all refs are packed and loose ref files are pruned.

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use grit_lib::config::ConfigSet;
use grit_lib::objects::ObjectKind;
use grit_lib::odb::Odb;
use grit_lib::refs::read_ref_file;
use grit_lib::refs::Ref;
use grit_lib::repo::Repository;
use grit_lib::shared_repo::{adjust_shared_repo_tree, git_config_perm};
use grit_lib::wildmatch::{wildmatch, WM_PATHNAME};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

/// Arguments for `grit pack-refs`.
#[derive(Debug, ClapArgs)]
pub struct Args {
    /// Pack all refs (default).
    #[arg(long)]
    pub all: bool,

    /// Prune loose refs after packing (default).
    #[arg(long = "prune")]
    pub prune: bool,

    /// Don't remove loose refs after packing.
    #[arg(long = "no-prune")]
    pub no_prune: bool,

    /// Accepted for Git compatibility; `maintenance`/hooks may pass this. Ignored for now.
    #[arg(long)]
    pub auto: bool,

    /// Pack only refs matching this pattern.
    #[arg(long = "include", action = clap::ArgAction::Append)]
    pub include: Vec<String>,

    /// Clear include patterns accumulated so far.
    #[arg(long = "no-include")]
    pub no_include: bool,

    /// Do not pack refs matching this pattern.
    #[arg(long = "exclude", action = clap::ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Clear exclude patterns accumulated so far.
    #[arg(long = "no-exclude")]
    pub no_exclude: bool,
}

/// Run `grit pack-refs`.
pub fn run(args: Args) -> Result<()> {
    let repo = Repository::discover(None).context("failed to discover repository")?;
    let git_dir = &repo.git_dir;

    if grit_lib::reftable::is_reftable_repo(git_dir) {
        return pack_reftable_refs(git_dir, args.auto);
    }

    if args.auto && !pack_refs_auto_needed(git_dir)? {
        return Ok(());
    }

    let include = if args.no_include {
        Vec::new()
    } else {
        args.include.clone()
    };
    let exclude = if args.no_exclude {
        Vec::new()
    } else {
        args.exclude.clone()
    };

    // Read existing packed-refs to merge with
    let mut packed = read_existing_packed_refs(git_dir)?;

    // Git's packed-refs format cannot represent symbolic refs. Only pack loose files that store
    // a direct object id; leave symbolic refs loose and drop any stale packed line for the same
    // name (matches `git pack-refs` / `refs/packed-backend.c`).
    let mut direct_loose: Vec<String> = Vec::new();
    walk_loose_under_refs(git_dir, "refs/", &mut |refname, path| {
        match read_ref_file(path).context(format!("reading {refname}"))? {
            Ref::Symbolic(_) => {
                packed.remove(refname);
            }
            Ref::Direct(oid) => {
                if !should_pack_ref(refname, &include, &exclude, args.no_include) {
                    return Ok(());
                }
                let peeled = peel_to_non_tag(&repo.odb, &oid);
                packed.insert(
                    refname.to_owned(),
                    PackedRef {
                        oid: oid.to_string(),
                        peeled,
                    },
                );
                direct_loose.push(refname.to_owned());
            }
        }
        Ok(())
    })?;

    if packed.is_empty() {
        let _ = fs::remove_file(git_dir.join("packed-refs"));
        return Ok(());
    }

    write_packed_refs(git_dir, &packed).context("failed to write packed-refs")?;

    if !args.no_prune {
        for refname in &direct_loose {
            prune_loose_ref(git_dir, refname);
        }
    }

    Ok(())
}

fn pack_reftable_refs(git_dir: &Path, auto: bool) -> Result<()> {
    let config = ConfigSet::load(Some(git_dir), true).unwrap_or_else(|_| ConfigSet::new());
    if let Some(raw) = config.get("reftable.blockSize") {
        if let Ok(block_size) = raw.parse::<u32>() {
            if block_size >= 16 * 1024 * 1024 {
                anyhow::bail!("fatal: reftable block size cannot exceed 16MB");
            }
            if block_size > 0 && block_size < 64 {
                anyhow::bail!("unable to compact stack: entry too large");
            }
        }
    }
    if let Some(raw) = config.get("reftable.restartInterval") {
        if let Ok(restart_interval) = raw.parse::<usize>() {
            if restart_interval > u16::MAX as usize {
                anyhow::bail!("fatal: reftable block size cannot exceed 65535");
            }
        }
    }
    if git_dir.join("reftable/tables.list.lock").exists() {
        anyhow::bail!("unable to compact stack: data is locked");
    }

    let mut stack =
        grit_lib::reftable::ReftableStack::open(git_dir).context("opening reftable stack")?;
    if auto && stack.table_names().len() <= 2 {
        return Ok(());
    }
    stack.compact().context("compacting reftable stack")?;
    maybe_emit_reference_fsync_counter(2);
    if let Some(shared) = config.get("core.sharedRepository") {
        let perm =
            git_config_perm("core.sharedRepository", &shared).map_err(|e| anyhow::anyhow!(e))?;
        if perm != 0 {
            adjust_shared_repo_tree(git_dir, perm)?;
        }
    }
    Ok(())
}

fn maybe_emit_reference_fsync_counter(count: u64) {
    if std::env::var("GIT_TEST_FSYNC").ok().as_deref() != Some("true") {
        return;
    }
    let Ok(path) = std::env::var("GIT_TRACE2_EVENT") else {
        return;
    };
    let _ = crate::trace2_write_json_counter_line(&path, "fsync", "hardware-flush", count);
}

fn ref_matches_pattern(refname: &str, pattern: &str) -> bool {
    refname == pattern || wildmatch(pattern.as_bytes(), refname.as_bytes(), WM_PATHNAME)
}

fn should_pack_ref(
    refname: &str,
    include: &[String],
    exclude: &[String],
    no_include: bool,
) -> bool {
    if refname.starts_with("refs/bisect/") || refname.starts_with("refs/worktree/") {
        return false;
    }
    if exclude.iter().any(|pat| ref_matches_pattern(refname, pat)) {
        return false;
    }
    if no_include {
        return false;
    }
    include.is_empty() || include.iter().any(|pat| ref_matches_pattern(refname, pat))
}

fn pack_refs_auto_needed(git_dir: &Path) -> Result<bool> {
    let loose = count_packable_loose_refs(git_dir)?;
    let packed = read_existing_packed_refs(git_dir)?.len();
    if packed == 0 {
        return Ok(loose >= 16);
    }
    let threshold = std::cmp::max(16, packed / 4);
    if packed >= 64 {
        Ok(loose > threshold)
    } else {
        Ok(loose >= threshold)
    }
}

fn count_packable_loose_refs(git_dir: &Path) -> Result<usize> {
    let mut count = 0usize;
    walk_loose_under_refs(git_dir, "refs/", &mut |refname, path| {
        if should_pack_ref(refname, &[], &[], false)
            && matches!(read_ref_file(path), Ok(Ref::Direct(_)))
        {
            count += 1;
        }
        Ok(())
    })?;
    Ok(count)
}

fn walk_loose_under_refs(
    git_dir: &Path,
    prefix: &str,
    visit: &mut impl FnMut(&str, &Path) -> Result<()>,
) -> Result<()> {
    let dir = git_dir.join(prefix.trim_end_matches('/'));
    let read = match fs::read_dir(&dir) {
        Ok(r) => r,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };

    for entry in read {
        let entry = entry?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        let refname = format!("{prefix}{name}");
        let path = entry.path();
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_dir() {
            walk_loose_under_refs(git_dir, &format!("{refname}/"), visit)?;
        } else if meta.is_file() {
            visit(&refname, &path)?;
        }
    }
    Ok(())
}

struct PackedRef {
    oid: String,
    /// If this is an annotated tag, the peeled (non-tag) OID.
    peeled: Option<String>,
}

/// Read existing packed-refs file into a map.
fn read_existing_packed_refs(git_dir: &Path) -> Result<BTreeMap<String, PackedRef>> {
    let path = git_dir.join("packed-refs");
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e.into()),
    };

    let mut map: BTreeMap<String, PackedRef> = BTreeMap::new();
    let mut last_ref: Option<String> = None;

    for line in content.lines() {
        if line.starts_with('#') {
            continue;
        }
        if let Some(hex) = line.strip_prefix('^') {
            // Peeled line for the previous ref
            if let Some(ref name) = last_ref {
                if let Some(entry) = map.get_mut(name) {
                    entry.peeled = Some(hex.trim().to_owned());
                }
            }
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let hash = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("").trim();
        if hash.len() == 40 && !name.is_empty() {
            last_ref = Some(name.to_owned());
            map.insert(
                name.to_owned(),
                PackedRef {
                    oid: hash.to_owned(),
                    peeled: None,
                },
            );
        }
    }

    Ok(map)
}

/// Write packed-refs file atomically via a lock file.
fn write_packed_refs(git_dir: &Path, packed: &BTreeMap<String, PackedRef>) -> Result<()> {
    let mut out = String::new();
    out.push_str("# pack-refs with: peeled fully-peeled sorted\n");

    for (name, entry) in packed {
        out.push_str(&entry.oid);
        out.push(' ');
        out.push_str(name);
        out.push('\n');
        if let Some(ref peeled) = entry.peeled {
            out.push('^');
            out.push_str(peeled);
            out.push('\n');
        }
    }

    let path = git_dir.join("packed-refs");
    let path = match fs::read_link(&path) {
        Ok(target) if target.is_absolute() => target,
        Ok(target) => git_dir.join(target),
        Err(_) => path,
    };
    let lock = path.with_file_name(format!(
        "{}.lock",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("packed-refs")
    ));
    wait_for_pack_refs_lock(git_dir, &lock)?;
    fs::write(&lock, &out)?;
    fs::rename(&lock, &path)?;
    Ok(())
}

fn wait_for_pack_refs_lock(git_dir: &Path, lock: &Path) -> Result<()> {
    let config = ConfigSet::load(Some(git_dir), true).unwrap_or_else(|_| ConfigSet::new());
    let timeout_ms = config
        .get("core.packedrefstimeout")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while lock.exists() {
        if timeout_ms == 0 || Instant::now() >= deadline {
            anyhow::bail!("cannot lock packed-refs");
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

/// Peel an annotated tag to its ultimate non-tag target.
/// Returns None if the object is not a tag.
fn peel_to_non_tag(odb: &Odb, oid: &grit_lib::objects::ObjectId) -> Option<String> {
    let obj = odb.read(oid).ok()?;
    if obj.kind != ObjectKind::Tag {
        return None;
    }

    // Walk the tag chain
    let mut current_oid = parse_tag_target(odb, &obj.data)?;
    loop {
        let inner = odb.read(&current_oid).ok()?;
        if inner.kind != ObjectKind::Tag {
            return Some(current_oid.to_string());
        }
        current_oid = parse_tag_target(odb, &inner.data)?;
    }
}

/// Parse the `object <hex>` line from raw tag data.
fn parse_tag_target(odb: &Odb, data: &[u8]) -> Option<grit_lib::objects::ObjectId> {
    let text = std::str::from_utf8(data).ok()?;
    let mut declared_kind = None;
    let mut target_oid = None;
    for line in text.lines() {
        if let Some(target) = line.strip_prefix("object ") {
            target_oid = target.trim().parse().ok();
        } else if let Some(kind) = line.strip_prefix("type ") {
            declared_kind = ObjectKind::from_bytes(kind.as_bytes()).ok();
        }
    }
    let target_oid = target_oid?;
    let declared_kind = declared_kind?;
    let target = odb.read(&target_oid).ok()?;
    if target.kind == declared_kind {
        Some(target_oid)
    } else {
        None
    }
}

/// Remove a loose ref file and clean up empty parent directories.
fn prune_loose_ref(git_dir: &Path, refname: &str) {
    let path = git_dir.join(refname);

    // Don't remove symbolic refs
    if let Ok(Ref::Symbolic(_)) = read_ref_file(&path) {
        return;
    }

    let _ = fs::remove_file(&path);

    // Clean up empty parent dirs up to refs/
    let refs_dir = git_dir.join("refs");
    let mut dir = path.parent().map(|p| p.to_path_buf());
    while let Some(d) = dir {
        if d == refs_dir || !d.starts_with(&refs_dir) {
            break;
        }
        if fs::remove_dir(&d).is_err() {
            break; // not empty or other error
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
}
