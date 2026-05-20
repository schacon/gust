//! `grit var` — print a Git logical variable.
//!
//! Implements `git var (-l | <variable>)`:
//! - Without flags: print the value of the named variable and exit 0.
//! - Without flags and variable has no value: exit 1 silently.
//! - With `-l`: list all config entries followed by all logical variables.

use anyhow::{bail, Result};
use clap::Args as ClapArgs;
use grit_lib::config::{global_config_paths_pub, ConfigSet};
use std::io::{self, Write};
use time::OffsetDateTime;

use crate::ident::{peek_name, resolve_email_lenient, resolve_name, IdentRole};

/// Arguments for `grit var`.
#[derive(Debug, ClapArgs)]
#[command(about = "Show a Git logical variable")]
pub struct Args {
    /// List all logical variables and config entries.
    #[arg(short = 'l', long = "list", conflicts_with = "variable")]
    pub list: bool,

    /// The variable name to query.
    pub variable: Option<String>,
}

/// Run the `var` command.
pub fn run(args: Args) -> Result<()> {
    if !args.list && args.variable.is_none() {
        bail!("usage: git var (-l | <variable>)");
    }

    // Try to discover a repository; config loading works without one too.
    let git_dir = grit_lib::repo::Repository::discover(None)
        .ok()
        .map(|r| r.git_dir);

    let config = ConfigSet::load(git_dir.as_deref(), true).unwrap_or_default();

    let stdout = io::stdout();
    let mut out = stdout.lock();

    if args.list {
        // Print all config entries first (mirrors `repo_config(show_config, …)`).
        for entry in config.entries() {
            match &entry.value {
                Some(v) => writeln!(out, "{}={}", entry.key, v)?,
                None => writeln!(out, "{}", entry.key)?,
            }
        }
        // Then print each logical variable.
        list_all_vars(&config, &mut out)?;
        return Ok(());
    }

    let var_name = args.variable.as_deref().unwrap_or("");
    let value = read_var(var_name, &config, true)?;
    match value {
        Some(v) => {
            writeln!(out, "{}", v)?;
            Ok(())
        }
        // Variable has no value → exit 1 with no output (matches Git behaviour).
        None => std::process::exit(1),
    }
}

/// Print every known logical variable (for `git var -l`).
fn list_all_vars(config: &ConfigSet, out: &mut impl Write) -> Result<()> {
    let vars: &[(&str, bool)] = &[
        ("GIT_COMMITTER_IDENT", false),
        ("GIT_AUTHOR_IDENT", false),
        ("GIT_EDITOR", false),
        ("GIT_SEQUENCE_EDITOR", false),
        ("GIT_PAGER", false),
        ("GIT_DEFAULT_BRANCH", false),
        ("GIT_SHELL_PATH", false),
        ("GIT_ATTR_SYSTEM", false),
        ("GIT_ATTR_GLOBAL", false),
        ("GIT_CONFIG_SYSTEM", false),
        ("GIT_CONFIG_GLOBAL", true), // multivalued — may contain '\n'
    ];

    for (name, multivalued) in vars {
        // Use non-strict mode for listing so missing idents are silently skipped.
        if let Ok(Some(val)) = read_var(name, config, false) {
            if *multivalued {
                for line in val.split('\n').filter(|s| !s.is_empty()) {
                    writeln!(out, "{}={}", name, line)?;
                }
            } else {
                writeln!(out, "{}={}", name, val)?;
            }
        }
    }
    Ok(())
}

/// Read one logical variable.
///
/// Returns `Ok(Some(value))` when found, `Ok(None)` when the variable has no
/// value (caller should exit 1), or `Err(…)` for an unrecognised variable name.
///
/// When `strict` is `true`, identity variables fail if name/email is absent.
fn read_var(name: &str, config: &ConfigSet, strict: bool) -> Result<Option<String>> {
    match name {
        "GIT_AUTHOR_IDENT" => author_ident(config, strict),
        "GIT_COMMITTER_IDENT" => committer_ident(config, strict),
        "GIT_EDITOR" => Ok(git_editor(config)),
        "GIT_SEQUENCE_EDITOR" => Ok(git_sequence_editor(config)),
        "GIT_PAGER" => Ok(Some(git_pager(config))),
        "GIT_DEFAULT_BRANCH" => Ok(Some(git_default_branch(config))),
        "GIT_SHELL_PATH" => Ok(git_shell_path()),
        "GIT_ATTR_SYSTEM" => Ok(git_attr_system()),
        "GIT_ATTR_GLOBAL" => Ok(git_attr_global()),
        "GIT_CONFIG_SYSTEM" => Ok(git_config_system()),
        "GIT_CONFIG_GLOBAL" => Ok(git_config_global()),
        _ => bail!("usage: git var (-l | <variable>)"),
    }
}

// ── Identity helpers ─────────────────────────────────────────────────────────

/// Build the author identity string from env/config.
fn author_ident(config: &ConfigSet, strict: bool) -> Result<Option<String>> {
    if strict {
        return strict_role_ident(config, IdentRole::Author, "GIT_AUTHOR_DATE");
    }
    let name = peek_name(config, IdentRole::Author);
    let email = resolve_email_lenient(config, IdentRole::Author);
    let date = std::env::var("GIT_AUTHOR_DATE")
        .ok()
        .unwrap_or_else(|| format_git_timestamp(OffsetDateTime::now_utc()));
    build_ident(name, email, date, false, "author")
}

/// Build the committer identity string from env/config.
fn committer_ident(config: &ConfigSet, strict: bool) -> Result<Option<String>> {
    if strict {
        return strict_role_ident(config, IdentRole::Committer, "GIT_COMMITTER_DATE");
    }
    let name = peek_name(config, IdentRole::Committer);
    let email = resolve_email_lenient(config, IdentRole::Committer);
    let date = std::env::var("GIT_COMMITTER_DATE")
        .ok()
        .unwrap_or_else(|| format_git_timestamp(OffsetDateTime::now_utc()));
    build_ident(name, email, date, false, "committer")
}

/// Email for `git var` with `IDENT_STRICT`: no passwd/synthetic auto-detection (t0007).
fn resolve_email_for_var_strict(config: &ConfigSet, role: IdentRole) -> Option<String> {
    let env_key = match role {
        IdentRole::Author => "GIT_AUTHOR_EMAIL",
        IdentRole::Committer => "GIT_COMMITTER_EMAIL",
    };
    let config_key = match role {
        IdentRole::Author => "author.email",
        IdentRole::Committer => "committer.email",
    };
    if std::env::var(env_key)
        .ok()
        .is_some_and(|v| !v.trim().is_empty())
    {
        return crate::ident::resolve_email(config, role).ok();
    }
    if config
        .get(config_key)
        .is_some_and(|v| !v.trim().is_empty())
    {
        return crate::ident::resolve_email(config, role).ok();
    }
    if config.get("user.email").is_some_and(|v| !v.trim().is_empty()) {
        return crate::ident::resolve_email(config, role).ok();
    }
    if std::env::var("EMAIL")
        .ok()
        .is_some_and(|v| !v.trim().is_empty())
    {
        return crate::ident::resolve_email(config, role).ok();
    }
    None
}

/// `git var <IDENT>` uses `IDENT_STRICT` (see upstream `builtin/var.c`).
fn strict_role_ident(
    config: &ConfigSet,
    role: IdentRole,
    date_env: &str,
) -> Result<Option<String>> {
    let name = match resolve_name(config, role) {
        Ok(n) => n,
        Err(_) => return Ok(None),
    };
    let Some(email) = resolve_email_for_var_strict(config, role) else {
        return Ok(None);
    };
    let date = std::env::var(date_env)
        .ok()
        .unwrap_or_else(|| format_git_timestamp(OffsetDateTime::now_utc()));
    Ok(Some(format!("{name} <{email}> {date}")))
}

/// Assemble `Name <email> timestamp tz` or error if `strict` and name missing.
fn build_ident(
    name: Option<String>,
    email: String,
    date: String,
    strict: bool,
    role: &str,
) -> Result<Option<String>> {
    let name = name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
    let email = email.trim().to_owned();

    if strict && name.is_none() {
        bail!(
                "*** Please tell me who you are.\n\n\
                 Run\n\n  git config user.email \"you@example.com\"\n  git config user.name \"Your Name\"\n\n\
                 to set your account's default identity.\n\
                 Omit --global to set the identity only in this repository.\n\n\
                 fatal: unable to auto-detect {role} name"
            );
    }

    match name {
        Some(n) => Ok(Some(format!("{n} <{email}> {date}"))),
        None => Ok(None),
    }
}

/// Format a UTC timestamp in Git's native `<epoch> <tz>` format.
fn format_git_timestamp(dt: OffsetDateTime) -> String {
    let epoch = dt.unix_timestamp();
    let offset = dt.offset();
    let hours = offset.whole_hours();
    let minutes = offset.minutes_past_hour().unsigned_abs();
    format!("{epoch} {hours:+03}{minutes:02}")
}

// ── Editor / pager ───────────────────────────────────────────────────────────

/// Resolve the editor like Git's `git_editor()` (see [`crate::editor::resolve_git_editor`]).
fn git_editor(config: &ConfigSet) -> Option<String> {
    crate::editor::resolve_git_editor(config, false)
}

/// Resolve the sequence editor: GIT_SEQUENCE_EDITOR env → sequence.editor config → GIT_EDITOR.
fn git_sequence_editor(config: &ConfigSet) -> Option<String> {
    std::env::var("GIT_SEQUENCE_EDITOR")
        .ok()
        .or_else(|| config.get("sequence.editor"))
        .or_else(|| git_editor(config))
}

/// Resolve the pager: GIT_PAGER env → core.pager config → PAGER env → "cat".
fn git_pager(config: &ConfigSet) -> String {
    std::env::var("GIT_PAGER")
        .ok()
        .or_else(|| config.get("core.pager"))
        .or_else(|| std::env::var("PAGER").ok())
        .unwrap_or_else(|| "cat".to_owned())
}

// ── Misc variables ───────────────────────────────────────────────────────────

/// Resolve the default branch name (matches `repo_default_branch_name` / `git init`).
fn git_default_branch(config: &ConfigSet) -> String {
    if let Ok(b) = std::env::var("GIT_TEST_DEFAULT_INITIAL_BRANCH_NAME") {
        if !b.is_empty() {
            return b;
        }
    }
    config
        .get("init.defaultbranch")
        .or_else(|| config.get("init.defaultBranch"))
        .unwrap_or_else(|| "main".to_owned())
}

/// Return the path to a POSIX-compatible shell.
fn git_shell_path() -> Option<String> {
    // Prefer /bin/sh; fall back to common alternatives.
    for candidate in &["/bin/sh", "/usr/bin/sh", "/usr/local/bin/sh"] {
        if std::path::Path::new(candidate).exists() {
            return Some((*candidate).to_owned());
        }
    }
    None
}

// ── Attribute / config path variables ────────────────────────────────────────

/// Return the system gitattributes path, or None if `GIT_ATTR_NOSYSTEM=1`.
fn git_attr_system() -> Option<String> {
    if std::env::var("GIT_ATTR_NOSYSTEM").as_deref() == Ok("1") {
        return None;
    }
    // Standard system gitattributes location on Linux/macOS.
    Some("/etc/gitattributes".to_owned())
}

/// Return the global gitattributes path.
///
/// Uses `$XDG_CONFIG_HOME/git/attributes` when XDG_CONFIG_HOME is set,
/// otherwise `$HOME/.config/git/attributes`.
fn git_attr_global() -> Option<String> {
    let xdg = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty());
    let base = if let Some(xdg) = xdg {
        xdg
    } else {
        let home = std::env::var("HOME").ok()?;
        format!("{home}/.config")
    };
    Some(format!("{base}/git/attributes"))
}

/// Return the system gitconfig path, or None if `GIT_CONFIG_NOSYSTEM=1`.
fn git_config_system() -> Option<String> {
    if std::env::var("GIT_CONFIG_NOSYSTEM").as_deref() == Ok("1") {
        return None;
    }
    // GIT_CONFIG_SYSTEM env can override the path.
    if let Ok(path) = std::env::var("GIT_CONFIG_SYSTEM") {
        return Some(path);
    }
    Some("/etc/gitconfig".to_owned())
}

/// Return the global gitconfig path(s) as a newline-joined string (multivalued).
fn git_config_global() -> Option<String> {
    let paths = global_config_paths_pub();
    if paths.is_empty() {
        None
    } else {
        Some(
            paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}
