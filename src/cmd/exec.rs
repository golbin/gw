use crate::cli::ExecArgs;
use crate::{Context, GwError, Result};
use std::thread;

pub fn exec_cmd(ctx: &Context, args: ExecArgs) -> Result<()> {
    let cmd = args.cmd.join(" ");
    let target_all = args.all || args.worktrees.is_empty();

    let worktrees = ctx.git.worktrees().map_err(crate::git::git_error)?;
    let mut targets = Vec::new();

    if target_all {
        for wt in worktrees {
            if let Some(name) = super::worktree_name_with_config(ctx, &wt.path) {
                targets.push((name, wt.path));
            }
        }
    } else {
        for name in &args.worktrees {
            let wt = super::find_worktree(ctx, name)?
                .ok_or_else(|| GwError::new(1, "worktree not found"))?;
            targets.push((name.clone(), wt.path));
        }
    }

    let parallel = args.parallel && !args.fail_fast;

    if parallel {
        let mut handles = Vec::new();
        for (name, path) in targets {
            let cmd = cmd.clone();
            let handle = thread::spawn(move || {
                let ok = super::run_shell(&cmd, &path).unwrap_or(false);
                (name, ok)
            });
            handles.push(handle);
        }
        let mut failed = false;
        for handle in handles {
            if let Ok((name, ok)) = handle.join() {
                if !ok {
                    failed = true;
                    eprintln!("exec failed: {}", name);
                }
            } else {
                failed = true;
            }
        }
        if failed {
            return Err(GwError::new(1, "exec failed"));
        }
    } else {
        for (name, path) in targets {
            let ok = super::run_shell(&cmd, &path).unwrap_or(false);
            if !ok {
                eprintln!("exec failed: {}", name);
                if args.fail_fast {
                    return Err(GwError::new(1, "exec failed"));
                }
            }
        }
    }

    Ok(())
}
