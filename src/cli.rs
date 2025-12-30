use clap::{ArgAction, Args, Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(name = "gw", version, about = "git worktree helper")]
pub struct Cli {
    #[arg(short = 'v', long = "verbose", action = ArgAction::SetTrue)]
    pub verbose: bool,
    #[arg(short = 'q', long = "quiet", action = ArgAction::SetTrue)]
    pub quiet: bool,
    #[arg(long = "color", default_value = "auto")]
    pub color: String,
    #[arg(long = "json", action = ArgAction::SetTrue)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(visible_aliases = ["new", "a"])]
    Add(AddArgs),
    #[command(visible_aliases = ["rm", "d"])]
    Del(DelArgs),
    #[command(visible_alias = "ls")]
    List(ListArgs),
    #[command(visible_alias = "st")]
    Status(StatusArgs),
    #[command(visible_aliases = ["merge", "ap"])]
    Apply(ApplyArgs),
    #[command(visible_aliases = ["sy"])]
    Sync(SyncArgs),
    #[command(visible_aliases = ["v"])]
    Verify(VerifyArgs),
    #[command(visible_aliases = ["n"])]
    Note(NoteArgs),
    #[command(visible_aliases = ["show", "i"])]
    Info(InfoArgs),
    #[command(visible_alias = "lk")]
    Lock(LockArgs),
    #[command(visible_alias = "ul")]
    Unlock(UnlockArgs),
    #[command(visible_alias = "g")]
    Gc(GcArgs),
    #[command(visible_alias = "c")]
    Cd(CdArgs),
    #[command(visible_alias = "x")]
    Exec(ExecArgs),
    Completion(CompletionArgs),
    ShellInit(ShellInitArgs),
}

#[derive(Args)]
pub struct AddArgs {
    pub name: String,
    #[arg(short = 'b', long = "base")]
    pub base: Option<String>,
    #[arg(short = 'B', long = "branch")]
    pub branch: Option<String>,
    #[arg(long = "path")]
    pub path: Option<String>,
}

#[derive(Args)]
pub struct DelArgs {
    pub name: String,
    #[arg(short = 'f', long = "force", action = ArgAction::SetTrue)]
    pub force: bool,
    #[arg(short = 'D', long = "delete-branch", action = ArgAction::SetTrue)]
    pub delete_branch: bool,
}

#[derive(Args, Default)]
pub struct ListArgs {
    #[arg(short = 'v', long = "verbose", action = ArgAction::SetTrue)]
    pub verbose: bool,
}

#[derive(Args, Default)]
pub struct StatusArgs {
    #[arg(long = "changes-detail", action = ArgAction::SetTrue)]
    pub changes_detail: bool,
    #[arg(long = "recent", default_value_t = 3)]
    pub recent: usize,
}

#[derive(Args)]
pub struct ApplyArgs {
    pub name: String,
    #[arg(short = 't', long = "target")]
    pub target: Option<String>,
    #[arg(long = "merge", action = ArgAction::SetTrue)]
    pub merge: bool,
    #[arg(long = "squash", action = ArgAction::SetTrue)]
    pub squash: bool,
    #[arg(long = "rebase", action = ArgAction::SetTrue)]
    pub rebase: bool,
    #[arg(short = 'c', long = "cleanup", action = ArgAction::SetTrue)]
    pub cleanup: bool,
}

#[derive(Args)]
pub struct SyncArgs {
    pub name: Option<String>,
    #[arg(long = "base")]
    pub base: Option<String>,
    #[arg(long = "rebase", action = ArgAction::SetTrue)]
    pub rebase: bool,
    #[arg(long = "merge", action = ArgAction::SetTrue)]
    pub merge: bool,
    #[arg(short = 'A', long = "all", action = ArgAction::SetTrue)]
    pub all: bool,
}

#[derive(Args)]
pub struct VerifyArgs {
    pub name: String,
}

#[derive(Args)]
pub struct NoteArgs {
    pub name: String,
    pub text: String,
}

#[derive(Args)]
pub struct InfoArgs {
    pub name: String,
}

#[derive(Args)]
pub struct LockArgs {
    pub name: String,
}

#[derive(Args)]
pub struct UnlockArgs {
    pub name: String,
}

#[derive(Args)]
pub struct GcArgs {
    #[arg(long = "prune", action = ArgAction::SetTrue)]
    pub prune: bool,
}

#[derive(Args)]
pub struct CdArgs {
    pub name: Option<String>,
    #[arg(long = "shell", action = ArgAction::SetTrue)]
    pub shell: bool,
}

#[derive(Args)]
pub struct ExecArgs {
    #[arg(short = 'A', long = "all", action = ArgAction::SetTrue)]
    pub all: bool,
    #[arg(short = 'w', long = "worktree")]
    pub worktrees: Vec<String>,
    #[arg(long = "parallel", action = ArgAction::SetTrue)]
    pub parallel: bool,
    #[arg(long = "fail-fast", action = ArgAction::SetTrue)]
    pub fail_fast: bool,
    #[arg(required = true, trailing_var_arg = true)]
    pub cmd: Vec<String>,
}

#[derive(Args)]
pub struct CompletionArgs {
    pub shell: Shell,
}

#[derive(Args)]
pub struct ShellInitArgs {
    pub shell: Option<Shell>,
    #[arg(long = "install", action = ArgAction::SetTrue)]
    pub install: bool,
    #[arg(long = "apply", action = ArgAction::SetTrue)]
    pub apply: bool,
}
