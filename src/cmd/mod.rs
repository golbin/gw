mod exec;

use crate::cli::{
    AddArgs, CdArgs, DelArgs, GcArgs, InfoArgs, ListArgs, LockArgs, NoteArgs, StatusArgs,
    SyncArgs, UnlockArgs, VerifyArgs, ApplyArgs,
};
use crate::{Context, GwError, Result};
use crate::git::{git_error, Worktree};
use chrono::{DateTime, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub use exec::exec_cmd;

pub fn add(ctx: &Context, args: AddArgs) -> Result<()> {
    let worktrees_dir = ctx.repo_root.join(ctx.config.worktrees_dir());
    let name = args.name;
    let path = args
        .path
        .map(PathBuf::from)
        .unwrap_or_else(|| worktrees_dir.join(&name));

    if path.exists() {
        return Err(GwError::new(1, "worktree path already exists"));
    }

    let branch = args
        .branch
        .unwrap_or_else(|| format!("{}{}", ctx.config.branch_prefix(), name));
    let base = if let Some(base) = args.base {
        base
    } else {
        ctx.git
            .resolve_base(&ctx.repo_root, ctx.config.default_base())
            .map_err(git_error)?
    };

    let mut cmd_args = vec!["worktree", "add"];
    let path_str = path.to_string_lossy().to_string();
    if ctx.git.branch_exists(&branch) {
        cmd_args.push(&path_str);
        cmd_args.push(&branch);
    } else {
        cmd_args.push("-b");
        cmd_args.push(&branch);
        cmd_args.push(&path_str);
        cmd_args.push(&base);
    }

    ctx.git.run(&cmd_args).map_err(git_error)?;

    let mut meta = ctx.meta.clone();
    meta.set_created(&name);
    meta.save().map_err(|e| GwError::new(1, e.to_string()))?;

    if !ctx.quiet {
        println!(
            "created: {} (branch: {}, base: {})",
            path.display(),
            branch,
            base
        );
    }

    Ok(())
}

pub fn del(ctx: &Context, args: DelArgs) -> Result<()> {
    let name = args.name;
    if is_locked(&ctx.repo_root, &name) {
        return Err(GwError::new(1, "worktree is locked"));
    }

    let worktree = find_worktree(ctx, &name)?;
    if worktree.is_none() {
        return Err(GwError::new(1, "worktree not found"));
    }
    let worktree = worktree.unwrap();

    if !args.force {
        let dirty = dirty_files(&ctx.git, &worktree.path).map_err(git_error)?;
        if dirty.total > 0 {
            return Err(GwError::new(1, "worktree is dirty (use --force)"));
        }
    }

    let mut cmd_args = vec!["worktree", "remove"];
    if args.force {
        cmd_args.push("--force");
    }
    let worktree_path = worktree.path.to_string_lossy().to_string();
    cmd_args.push(&worktree_path);
    ctx.git.run(&cmd_args).map_err(git_error)?;

    if args.delete_branch {
        if let Some(branch) = worktree.branch {
            let branch = branch.trim_start_matches("refs/heads/");
            let _ = ctx.git.run(&["branch", "-D", branch]);
        }
    }

    let mut meta = ctx.meta.clone();
    meta.remove(&name);
    meta.save().map_err(|e| GwError::new(1, e.to_string()))?;

    Ok(())
}

pub fn list(ctx: &Context, _args: ListArgs) -> Result<()> {
    let worktrees = ctx.git.worktrees().map_err(git_error)?;
    let root = ctx
        .repo_root
        .canonicalize()
        .unwrap_or_else(|_| ctx.repo_root.clone());
    let current = ctx
        .git
        .current_toplevel()
        .map_err(git_error)?
        .canonicalize()
        .unwrap_or_else(|_| root.clone());
    println!("CUR NAME     BRANCH     PATH");
    for wt in worktrees {
        let name = worktree_display_name(ctx, &wt.path, &root);
        let branch = wt
            .branch
            .as_ref()
            .map(|b| short_branch(b))
            .unwrap_or_default();
        let path = wt.path.to_string_lossy().to_string();
        let is_current = wt
            .path
            .canonicalize()
            .map(|p| p == current)
            .unwrap_or(false);
        let mark = if is_current { "*" } else { " " };
        println!("{}  {:<8} {:<10} {}", mark, name, branch, path);
    }
    Ok(())
}

pub fn status(ctx: &Context, args: StatusArgs) -> Result<()> {
    let worktrees = ctx.git.worktrees().map_err(git_error)?;
    let root = ctx
        .repo_root
        .canonicalize()
        .unwrap_or_else(|_| ctx.repo_root.clone());
    if ctx.json {
        let mut items = Vec::new();
        for wt in worktrees {
            let name = worktree_display_name(ctx, &wt.path, &root);
            let dirty = dirty_files(&ctx.git, &wt.path).map_err(git_error)?;
            let (commit_time, commit_subject) =
                last_commit_info(&ctx.git, &wt.path).unwrap_or((0, "".to_string()));
            let recent = recent_uncommitted(&ctx.git, &wt.path, args.recent);
            let last_change_time = recent.first().map(|(_, _, ts)| *ts).unwrap_or(0);
            let recent_items: Vec<_> = recent
                .iter()
                .map(|(file, status, ts)| {
                    serde_json::json!({
                        "file": file,
                        "status": status.to_string(),
                        "time": pretty_time(*ts),
                    })
                })
                .collect();
            items.push(serde_json::json!({
                "name": name,
                "branch": wt.branch.as_ref().map(|b| short_branch(b)),
                "changes": if args.changes_detail {
                    format!("{} ({}/{}/{})", dirty.total, dirty.staged, dirty.unstaged, dirty.untracked)
                } else {
                    dirty.total.to_string()
                },
                "last_commit_time": pretty_time(commit_time),
                "last_commit_subject": commit_subject,
                "last_change_time": pretty_time(last_change_time),
                "recent_files": recent_items,
            }));
        }
        println!("{}", serde_json::to_string_pretty(&items).unwrap_or("[]".to_string()));
        return Ok(());
    }

    let mut rows = Vec::new();
    let mut recent_map = Vec::new();
    for wt in worktrees {
        let name = worktree_display_name(ctx, &wt.path, &root);
        let dirty = dirty_files(&ctx.git, &wt.path).map_err(git_error)?;
        let (commit_time, commit_subject) =
            last_commit_info(&ctx.git, &wt.path).unwrap_or((0, "".to_string()));
        let commit_display = if commit_time == 0 {
            String::new()
        } else {
            let subject = truncate_text(&commit_subject, 20);
            format!("{} ({})", subject, pretty_time(commit_time))
        };
        let recent = recent_uncommitted(&ctx.git, &wt.path, args.recent);
        let last_change_time = recent.first().map(|(_, _, ts)| *ts).unwrap_or(0);
        let last_change_display = if last_change_time == 0 {
            "-".to_string()
        } else {
            pretty_time(last_change_time)
        };
        rows.push(vec![
            name,
            wt.branch
                .as_ref()
                .map(|b| short_branch(b))
                .unwrap_or_default(),
            format_changes(&dirty, args.changes_detail),
            last_change_display,
            commit_display,
            String::new(),
        ]);
        recent_map.push(recent);
    }

    let headers = vec![
        "NAME".to_string(),
        "BRANCH".to_string(),
        if args.changes_detail {
            "CHANGES (ST/UN/??)".to_string()
        } else {
            "CHANGES".to_string()
        },
        "LAST CHANGE".to_string(),
        "LAST COMMIT".to_string(),
        "RECENT FILES".to_string(),
    ];
    print_table_box(headers, rows, recent_map);
    Ok(())
}

pub fn apply(ctx: &Context, args: ApplyArgs) -> Result<()> {
    let worktree = find_worktree(ctx, &args.name)?
        .ok_or_else(|| GwError::new(1, "worktree not found"))?;
    let source_branch = worktree
        .branch
        .clone()
        .unwrap_or_else(|| args.name.clone());
    let source_branch = source_branch.trim_start_matches("refs/heads/").to_string();

    let target = if let Some(target) = args.target {
        target
    } else {
        ctx.git.current_branch(&ctx.repo_root).map_err(git_error)?
    };

    let dirty = dirty_files(&ctx.git, &ctx.repo_root).map_err(git_error)?;
    if dirty.total > 0 {
        return Err(GwError::new(1, "target worktree is dirty"));
    }

    ctx.git
        .run_in(&ctx.repo_root, &["checkout", &target])
        .map_err(git_error)?;

    let mode = merge_mode(args.merge, args.squash, args.rebase);
    let result = match mode.as_str() {
        "squash" => ctx
            .git
            .run_in(&ctx.repo_root, &["merge", "--squash", &source_branch]),
        "rebase" => ctx.git.run_in(&ctx.repo_root, &["rebase", &source_branch]),
        _ => ctx
            .git
            .run_in(&ctx.repo_root, &["merge", "--no-ff", &source_branch]),
    };

    if let Err(err) = result {
        return Err(GwError::new(4, format!("apply failed: {}", err)));
    }

    if args.cleanup {
        del(
            ctx,
            DelArgs {
                name: args.name,
                force: true,
                delete_branch: true,
            },
        )?;
    }

    Ok(())
}

pub fn sync(ctx: &Context, args: SyncArgs) -> Result<()> {
    ctx.git
        .run(&["fetch", "origin", "--prune"])
        .map_err(git_error)?;

    let worktrees = ctx.git.worktrees().map_err(git_error)?;
    let target_names: Vec<String> = if args.all {
        worktrees
            .iter()
            .filter_map(|wt| worktree_name_with_config(ctx, &wt.path))
            .collect()
    } else if let Some(name) = args.name.clone() {
        vec![name]
    } else {
        return Err(GwError::new(1, "sync requires <name> or --all"));
    };

    let base = if let Some(base) = args.base {
        base
    } else {
        ctx.git
            .resolve_base(&ctx.repo_root, ctx.config.default_base())
            .map_err(git_error)?
    };

    let mode = if args.merge { "merge" } else { "rebase" };

    for name in target_names {
        let wt = find_worktree(ctx, &name)?
            .ok_or_else(|| GwError::new(1, "worktree not found"))?;
        let result = match mode {
            "merge" => ctx
                .git
                .run_in(&wt.path, &["merge", &base]),
            _ => ctx
                .git
                .run_in(&wt.path, &["rebase", &base]),
        };
        if let Err(err) = result {
            return Err(GwError::new(4, format!("sync failed: {}", err)));
        }
    }

    Ok(())
}

pub fn verify(ctx: &Context, args: VerifyArgs) -> Result<()> {
    let wt = find_worktree(ctx, &args.name)?
        .ok_or_else(|| GwError::new(1, "worktree not found"))?;

    let mut commands = Vec::new();
    if wt.path.join("Cargo.toml").exists() {
        commands.push(ctx.config.verify_rust());
    }
    if wt.path.join("package.json").exists() {
        commands.push(ctx.config.verify_node());
    }
    if wt.path.join("pyproject.toml").exists() || wt.path.join("requirements.txt").exists() {
        commands.push(ctx.config.verify_python());
    }

    if commands.is_empty() {
        if !ctx.quiet {
            println!("verify: no commands to run");
        }
        return Ok(());
    }

    for cmd in commands {
        let status = run_shell(&cmd, &wt.path).map_err(|e| GwError::new(3, e))?;
        if !status {
            return Err(GwError::new(3, format!("verify failed: {}", cmd)));
        }
    }

    Ok(())
}

pub fn note(ctx: &Context, args: NoteArgs) -> Result<()> {
    let mut meta = ctx.meta.clone();
    meta.add_note(&args.name, args.text);
    meta.save().map_err(|e| GwError::new(1, e.to_string()))?;
    Ok(())
}

pub fn info(ctx: &Context, args: InfoArgs) -> Result<()> {
    let meta = ctx.meta.clone();
    if let Some(wt) = meta.get(&args.name) {
        if ctx.json {
            let out = serde_json::to_string_pretty(&wt).unwrap_or_else(|_| "{}".to_string());
            println!("{}", out);
        } else {
            println!("name: {}", args.name);
            println!("created_at: {}", wt.created_at.clone().unwrap_or_default());
            println!("created_by: {}", wt.created_by.clone().unwrap_or_default());
            println!("last_activity_at: {}", wt.last_activity_at.clone().unwrap_or_default());
            if !wt.notes.is_empty() {
                println!("notes:");
                for note in &wt.notes {
                    println!("- {}", note);
                }
            }
            if !wt.tags.is_empty() {
                println!("tags: {}", wt.tags.join(", "));
            }
        }
        Ok(())
    } else {
        Err(GwError::new(1, "no meta for worktree"))
    }
}

pub fn lock(ctx: &Context, args: LockArgs) -> Result<()> {
    let lock_path = lock_path(&ctx.repo_root, &args.name);
    fs::create_dir_all(lock_path.parent().unwrap()).map_err(|e| GwError::new(1, e.to_string()))?;
    fs::write(&lock_path, b"").map_err(|e| GwError::new(1, e.to_string()))?;
    Ok(())
}

pub fn unlock(ctx: &Context, args: UnlockArgs) -> Result<()> {
    let lock_path = lock_path(&ctx.repo_root, &args.name);
    if lock_path.exists() {
        fs::remove_file(&lock_path).map_err(|e| GwError::new(1, e.to_string()))?;
    }
    Ok(())
}

pub fn gc(ctx: &Context, args: GcArgs) -> Result<()> {
    let worktrees = ctx.git.worktrees().map_err(git_error)?;
    let meta = ctx.meta.clone();
    let mut candidates = Vec::new();

    for wt in worktrees {
        let name = match worktree_name_with_config(ctx, &wt.path) {
            Some(n) => n,
            None => continue,
        };
        if is_locked(&ctx.repo_root, &name) {
            continue;
        }
        let dirty = dirty_files(&ctx.git, &wt.path).map_err(git_error)?;
        let last_commit_time = last_commit_unix(&ctx.git, &wt.path).unwrap_or(0);
        let last_activity = meta
            .get(&name)
            .and_then(|m| m.last_activity_at.clone())
            .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
            .map(|dt| dt.timestamp())
            .unwrap_or(last_commit_time);
        let now = Utc::now().timestamp();
        let stale_days = ctx.config.gc_stale_days();
        let stale = now - last_activity >= stale_days * 24 * 60 * 60;
        if stale || (dirty.total == 0 && branch_merged(&ctx.git, &wt, &ctx.repo_root)) {
            candidates.push((name, wt.path));
        }
    }

    if candidates.is_empty() {
        if !ctx.quiet {
            println!("gc: no candidates");
        }
        return Ok(());
    }

    for (name, path) in candidates {
        if args.prune {
            let _ = ctx.git.run(&["worktree", "remove", "--force", path.to_string_lossy().as_ref()]);
            println!("pruned: {}", name);
        } else {
            println!("candidate: {}", name);
        }
    }

    Ok(())
}

pub fn cd(ctx: &Context, args: CdArgs) -> Result<()> {
    let mut target = ctx.repo_root.clone();
    if let Some(name) = args.name {
        if name == "root" {
            target = ctx.repo_root.clone();
        } else {
            let wt = find_worktree(ctx, &name)?
                .ok_or_else(|| GwError::new(1, "worktree not found"))?;
            target = wt.path;
        }
    }
    if args.shell {
        println!("cd \"{}\"", target.display());
    } else {
        println!("{}", target.display());
    }
    Ok(())
}

pub fn completion(args: crate::cli::CompletionArgs) -> Result<()> {
    use clap::CommandFactory;
    let mut cmd = crate::cli::Cli::command();
    clap_complete::generate(args.shell, &mut cmd, "gw", &mut std::io::stdout());
    Ok(())
}

pub fn shell_init(args: crate::cli::ShellInitArgs) -> Result<()> {
    use clap_complete::Shell;
    let shell = if let Some(shell) = args.shell {
        shell
    } else {
        detect_shell().ok_or_else(|| GwError::new(1, "could not detect shell"))?
    };

    let auto = args.shell.is_none() && !args.install && !args.apply;
    let install = args.install || auto;
    let apply = args.apply || auto;

    if install {
        install_shell_init(shell)?;
        if apply {
            let script = match shell {
                Shell::Bash => bash_init(),
                Shell::Zsh => bash_init(),
                Shell::Fish => fish_init(),
                Shell::PowerShell => powershell_init(),
                Shell::Elvish => "".to_string(),
                _ => "".to_string(),
            };
            print!("{}", script);
        } else {
            let hint = match shell {
                Shell::Bash => "eval \"$(gw shell-init bash)\"",
                Shell::Zsh => "eval \"$(gw shell-init zsh)\"",
                Shell::Fish => "gw shell-init fish | source",
                Shell::PowerShell => "gw shell-init powershell | Invoke-Expression",
                _ => "",
            };
            eprintln!(
                "Shell integration installed. Restart your shell or run: {}",
                hint
            );
        }
        return Ok(());
    }

    let script = match shell {
        Shell::Bash => bash_init(),
        Shell::Zsh => bash_init(),
        Shell::Fish => fish_init(),
        Shell::PowerShell => powershell_init(),
        Shell::Elvish => "".to_string(),
        _ => "".to_string(),
    };
    print!("{}", script);
    Ok(())
}

#[derive(Debug)]
#[derive(Clone)]
pub(crate) struct DirtyInfo {
    total: usize,
    staged: usize,
    unstaged: usize,
    untracked: usize,
}

pub(crate) fn dirty_files(
    git: &crate::git::Git,
    path: &Path,
) -> std::result::Result<DirtyInfo, String> {
    let out = git.run_in(path, &["status", "--porcelain"])?;
    let mut staged = 0;
    let mut unstaged = 0;
    let mut untracked = 0;
    for line in out.lines() {
        if line.starts_with("??") {
            untracked += 1;
        } else {
            if line.chars().nth(0).unwrap_or(' ') != ' ' {
                staged += 1;
            }
            if line.chars().nth(1).unwrap_or(' ') != ' ' {
                unstaged += 1;
            }
        }
    }
    Ok(DirtyInfo {
        total: staged + unstaged + untracked,
        staged,
        unstaged,
        untracked,
    })
}

fn last_commit_info(git: &crate::git::Git, path: &Path) -> Option<(i64, String)> {
    let out = git.run_in(path, &["log", "-1", "--format=%ct|%s"]).ok()?;
    let mut parts = out.trim().splitn(2, '|');
    let ts = parts.next().unwrap_or("").trim();
    let subject = parts.next().unwrap_or("").trim();
    let ts_num: i64 = ts.parse().unwrap_or(0);
    Some((ts_num, subject.to_string()))
}

fn last_commit_unix(git: &crate::git::Git, path: &Path) -> Option<i64> {
    let out = git.run_in(path, &["log", "-1", "--format=%ct"]).ok()?;
    out.trim().parse().ok()
}

pub(crate) fn worktree_name_with_config(ctx: &Context, path: &Path) -> Option<String> {
    let worktrees_dir = ctx.repo_root.join(ctx.config.worktrees_dir());
    let rel = path.strip_prefix(&worktrees_dir).ok()?;
    Some(rel.to_string_lossy().to_string())
}

fn worktree_display_name(ctx: &Context, path: &Path, root: &Path) -> String {
    if let Ok(canon) = path.canonicalize() {
        if canon == root {
            return "root".to_string();
        }
    }
    worktree_name_with_config(ctx, path).unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn merge_mode(merge: bool, squash: bool, rebase: bool) -> String {
    if squash {
        return "squash".to_string();
    }
    if rebase {
        return "rebase".to_string();
    }
    if merge {
        return "merge".to_string();
    }
    "merge".to_string()
}

pub(crate) fn run_shell(cmd: &str, dir: &Path) -> std::result::Result<bool, String> {
    let status = if cfg!(windows) {
        Command::new("cmd")
            .arg("/C")
            .arg(cmd)
            .current_dir(dir)
            .status()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(dir)
            .status()
    }
    .map_err(|e| format!("command failed: {}", e))?;
    Ok(status.success())
}

fn is_locked(repo_root: &Path, name: &str) -> bool {
    lock_path(repo_root, name).exists()
}

fn lock_path(repo_root: &Path, name: &str) -> PathBuf {
    repo_root.join(".gw").join("locks").join(format!("{}.lock", name))
}

pub(crate) fn find_worktree(ctx: &Context, name: &str) -> Result<Option<Worktree>> {
    let worktrees = ctx.git.worktrees().map_err(git_error)?;
    for wt in worktrees {
        if let Some(wt_name) = worktree_name_with_config(ctx, &wt.path) {
            if wt_name == name {
                return Ok(Some(wt));
            }
        }
    }
    Ok(None)
}

fn branch_merged(git: &crate::git::Git, wt: &Worktree, repo_root: &Path) -> bool {
    let base = git
        .resolve_base(repo_root, None)
        .unwrap_or_else(|_| "".to_string());
    let branch = wt
        .branch
        .clone()
        .unwrap_or_else(|| "".to_string())
        .trim_start_matches("refs/heads/")
        .to_string();
    if base.is_empty() || branch.is_empty() {
        return false;
    }
    git.run(&["branch", "--merged", &base])
        .map(|out| out.lines().any(|line| line.trim() == branch))
        .unwrap_or(false)
}

fn recent_uncommitted(
    git: &crate::git::Git,
    path: &Path,
    max: usize,
) -> Vec<(String, char, i64)> {
    let out = match git.run_in(path, &["status", "--porcelain"]) {
        Ok(out) => out,
        Err(_) => return Vec::new(),
    };
    let mut results = Vec::new();
    for line in out.lines() {
        let line = line.trim_end();
        if line.len() < 3 {
            continue;
        }
        let status = &line[..2];
        let file = line[3..].trim();
        if file.is_empty() {
            continue;
        }
        let status_char = if status == "??" {
            '?'
        } else if status.chars().next().unwrap_or(' ') != ' ' {
            status.chars().next().unwrap_or('?')
        } else {
            status.chars().nth(1).unwrap_or('?')
        };
        let time = file_mtime(path, file);
        results.push((file.to_string(), status_char, time));
    }
    results.sort_by(|a, b| b.2.cmp(&a.2));
    results.truncate(max);
    results
}

fn file_mtime(root: &Path, rel: &str) -> i64 {
    let full = root.join(rel);
    let meta = match std::fs::metadata(&full) {
        Ok(m) => m,
        Err(_) => return 0,
    };
    let modified = match meta.modified() {
        Ok(m) => m,
        Err(_) => return 0,
    };
    let since = match modified.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d,
        Err(_) => return 0,
    };
    since.as_secs() as i64
}

fn pretty_time(ts: i64) -> String {
    if ts <= 0 {
        return "-".to_string();
    }
    let now = Utc::now().timestamp();
    let diff = now.saturating_sub(ts);
    if diff < 60 {
        return "just now".to_string();
    }
    if diff < 3600 {
        return format!("{}m ago", diff / 60);
    }
    if diff < 86400 {
        return format!("{}h ago", diff / 3600);
    }
    if diff < 86400 * 7 {
        return format!("{}d ago", diff / 86400);
    }
    DateTime::<Utc>::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn short_branch(branch: &str) -> String {
    branch.trim_start_matches("refs/heads/").to_string()
}

fn format_changes(dirty: &DirtyInfo, detail: bool) -> String {
    if detail {
        format!(
            "{} ({}/{}/{})",
            dirty.total, dirty.staged, dirty.unstaged, dirty.untracked
        )
    } else {
        dirty.total.to_string()
    }
}

fn truncate_text(input: &str, max_width: usize) -> String {
    let mut width = 0usize;
    let mut out = String::new();
    for ch in input.chars() {
        let ch_width = char_width(ch);
        if width + ch_width > max_width {
            out.push_str("...");
            return out;
        }
        out.push(ch);
        width += ch_width;
    }
    out
}

fn char_width(ch: char) -> usize {
    if ch.is_ascii() {
        return 1;
    }
    let code = ch as u32;
    if (0x1100..=0x115F).contains(&code)
        || (0x2E80..=0xA4CF).contains(&code)
        || (0xAC00..=0xD7A3).contains(&code)
        || (0xF900..=0xFAFF).contains(&code)
        || (0xFE10..=0xFE6F).contains(&code)
        || (0xFF00..=0xFF60).contains(&code)
        || (0xFFE0..=0xFFE6).contains(&code)
    {
        return 2;
    }
    1
}

fn print_table_box(
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    recent: Vec<Vec<(String, char, i64)>>,
) {
    let mut widths = vec![0usize; headers.len()];
    for (idx, header) in headers.iter().enumerate() {
        widths[idx] = widths[idx].max(header.len());
    }
    for row in &rows {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(cell.len());
        }
    }
    if let Some(last_width) = widths.last_mut() {
        for files in &recent {
            for (file, status, ts) in files {
                let entry = format!("{} {} ({})", status, file, pretty_time(*ts));
                *last_width = (*last_width).max(entry.len());
            }
        }
    }
    let border = table_border(&widths);
    println!("{}", border);
    println!("{}", table_row(&headers, &widths));
    println!("{}", border);
    for (row, files) in rows.into_iter().zip(recent.into_iter()) {
        let base = row;
        let files = if files.is_empty() {
            vec![("-".to_string(), '-', 0)]
        } else {
            files
        };
        for (idx, (file, status, ts)) in files.into_iter().enumerate() {
            let mut current = base.clone();
            if idx == 0 {
                current[headers.len() - 1] =
                    format!("{} {} ({})", status, file, pretty_time(ts));
                println!("{}", table_row(&current, &widths));
            } else {
                for cell in current.iter_mut().take(headers.len() - 1) {
                    cell.clear();
                }
                current[headers.len() - 1] =
                    format!("{} {} ({})", status, file, pretty_time(ts));
                println!("{}", table_row(&current, &widths));
            }
        }
        println!("{}", border);
    }
}

fn pad_right(value: &str, width: usize) -> String {
    if value.len() >= width {
        return value.to_string();
    }
    let mut out = String::with_capacity(width);
    out.push_str(value);
    out.push_str(&" ".repeat(width - value.len()));
    out
}

fn table_border(widths: &[usize]) -> String {
    let mut line = String::new();
    line.push('+');
    for width in widths {
        line.push_str(&"-".repeat(*width + 2));
        line.push('+');
    }
    line
}

fn table_row(cells: &[String], widths: &[usize]) -> String {
    let mut line = String::new();
    line.push('|');
    for (cell, width) in cells.iter().zip(widths.iter()) {
        line.push(' ');
        line.push_str(&pad_right(cell, *width));
        line.push(' ');
        line.push('|');
    }
    line
}

fn bash_init() -> String {
    [
        "gw() {",
        "  if [ \"$1\" = \"cd\" ]; then",
        "    shift",
        "    cd \"$(command gw cd \"$@\")\"",
        "  else",
        "    command gw \"$@\"",
        "  fi",
        "}",
        "",
    ]
    .join("\n")
}

fn fish_init() -> String {
    [
        "function gw",
        "  if test (count $argv) -ge 1; and test $argv[1] = \"cd\"",
        "    set -e argv[1]",
        "    cd (command gw cd $argv)",
        "  else",
        "    command gw $argv",
        "  end",
        "end",
        "",
    ]
    .join("\n")
}

fn powershell_init() -> String {
    [
        "function gw {",
        "  param([Parameter(ValueFromRemainingArguments=$true)] $Args)",
        "  if ($Args.Count -gt 0 -and $Args[0] -eq 'cd') {",
        "    $target = $Args[1]",
        "    Set-Location (gw cd $target)",
        "  } else {",
        "    & gw.exe @Args",
        "  }",
        "}",
        "",
    ]
    .join("\n")
}

fn detect_shell() -> Option<clap_complete::Shell> {
    if cfg!(windows) {
        if std::env::var("PSModulePath").is_ok() {
            return Some(clap_complete::Shell::PowerShell);
        }
        return None;
    }
    let shell = std::env::var("SHELL").ok()?;
    if shell.ends_with("bash") {
        return Some(clap_complete::Shell::Bash);
    }
    if shell.ends_with("zsh") {
        return Some(clap_complete::Shell::Zsh);
    }
    if shell.ends_with("fish") {
        return Some(clap_complete::Shell::Fish);
    }
    None
}

fn install_shell_init(shell: clap_complete::Shell) -> Result<()> {
    match shell {
        clap_complete::Shell::Bash => install_append("~/.bashrc", "eval \"$(gw shell-init bash)\""),
        clap_complete::Shell::Zsh => install_append("~/.zshrc", "eval \"$(gw shell-init zsh)\""),
        clap_complete::Shell::Fish => install_fish(),
        clap_complete::Shell::PowerShell => install_powershell(),
        _ => Err(GwError::new(1, "unsupported shell for install")),
    }
}

fn install_append(path: &str, line: &str) -> Result<()> {
    let path = expand_home(path);
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    if content.contains(line) {
        return Ok(());
    }
    let mut new_content = content;
    if !new_content.ends_with('\n') && !new_content.is_empty() {
        new_content.push('\n');
    }
    new_content.push_str(line);
    new_content.push('\n');
    std::fs::write(&path, new_content).map_err(|e| GwError::new(1, e.to_string()))
}

fn install_fish() -> Result<()> {
    let path = expand_home("~/.config/fish/conf.d/gw.fish");
    std::fs::create_dir_all(path.parent().unwrap())
        .map_err(|e| GwError::new(1, e.to_string()))?;
    std::fs::write(&path, fish_init()).map_err(|e| GwError::new(1, e.to_string()))
}

fn install_powershell() -> Result<()> {
    let profile = std::env::var("PROFILE").ok();
    let path = if let Some(profile) = profile {
        std::path::PathBuf::from(profile)
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        std::path::PathBuf::from(home)
            .join("Documents")
            .join("PowerShell")
            .join("Microsoft.PowerShell_profile.ps1")
    } else {
        return Err(GwError::new(1, "unable to locate PowerShell profile"));
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| GwError::new(1, e.to_string()))?;
    }
    let line = "gw shell-init powershell | Invoke-Expression";
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    if content.contains(line) {
        return Ok(());
    }
    let mut new_content = content;
    if !new_content.ends_with('\n') && !new_content.is_empty() {
        new_content.push('\n');
    }
    new_content.push_str(line);
    new_content.push('\n');
    std::fs::write(&path, new_content).map_err(|e| GwError::new(1, e.to_string()))
}

fn expand_home(path: &str) -> std::path::PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(stripped);
        }
        if let Ok(home) = std::env::var("USERPROFILE") {
            return std::path::PathBuf::from(home).join(stripped);
        }
    }
    std::path::PathBuf::from(path)
}
