mod cli;
mod config;
mod git;
mod meta;
mod cmd;

use crate::cli::{Cli, Commands};
use crate::config::Config;
use crate::git::Git;
use crate::meta::MetaStore;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug)]
pub struct GwError {
    pub code: i32,
    pub message: String,
}

impl GwError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, GwError>;

#[derive(Clone)]
pub struct Context {
    pub repo_root: PathBuf,
    pub git: Git,
    pub config: Config,
    pub meta: MetaStore,
    pub verbose: bool,
    pub quiet: bool,
    pub json: bool,
    pub color: String,
}

fn main() {
    let code = match run() {
        Ok(()) => 0,
        Err(err) => {
            if !err.message.is_empty() {
                eprintln!("{}", err.message);
            }
            err.code
        }
    };
    std::process::exit(code);
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let git = Git::new();
    let repo_root = git.repo_root().map_err(|e| GwError::new(2, e))?;
    let config = Config::load(&repo_root).map_err(|e| GwError::new(1, e.to_string()))?;
    let meta = MetaStore::new(&repo_root).map_err(|e| GwError::new(1, e.to_string()))?;

    let ctx = Context {
        repo_root,
        git,
        config,
        meta,
        verbose: cli.verbose,
        quiet: cli.quiet,
        json: cli.json,
        color: cli.color,
    };

    match cli.command {
        Commands::Add(args) => cmd::add(&ctx, args),
        Commands::Del(args) => cmd::del(&ctx, args),
        Commands::List(args) => cmd::list(&ctx, args),
        Commands::Status(args) => cmd::status(&ctx, args),
        Commands::Apply(args) => cmd::apply(&ctx, args),
        Commands::Sync(args) => cmd::sync(&ctx, args),
        Commands::Verify(args) => cmd::verify(&ctx, args),
        Commands::Note(args) => cmd::note(&ctx, args),
        Commands::Info(args) => cmd::info(&ctx, args),
        Commands::Lock(args) => cmd::lock(&ctx, args),
        Commands::Unlock(args) => cmd::unlock(&ctx, args),
        Commands::Gc(args) => cmd::gc(&ctx, args),
        Commands::Cd(args) => cmd::cd(&ctx, args),
        Commands::Exec(args) => cmd::exec_cmd(&ctx, args),
        Commands::Completion(args) => cmd::completion(args),
        Commands::ShellInit(args) => cmd::shell_init(args),
    }
}
