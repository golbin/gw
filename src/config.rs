use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub gc: GcConfig,
    #[serde(default)]
    pub verify: VerifyConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Defaults {
    pub base: Option<String>,
    pub worktrees_dir: Option<String>,
    pub branch_prefix: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GcConfig {
    pub stale_days: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VerifyConfig {
    pub rust: Option<String>,
    pub node: Option<String>,
    pub python: Option<String>,
}

impl Config {
    pub fn load(repo_root: &Path) -> anyhow::Result<Self> {
        let mut config = Config::default();

        if let Some(global_path) = global_config_path() {
            if global_path.exists() {
                let data = fs::read_to_string(&global_path)?;
                let parsed: Config = toml::from_str(&data)?;
                config = merge(config, parsed);
            }
        }

        let project_path = repo_root.join(".gw").join("config.toml");
        if project_path.exists() {
            let data = fs::read_to_string(&project_path)?;
            let parsed: Config = toml::from_str(&data)?;
            config = merge(config, parsed);
        }

        Ok(config)
    }

    pub fn worktrees_dir(&self) -> String {
        if let Ok(value) = env::var("GW_WORKTREES_DIR") {
            return value;
        }
        self.defaults
            .worktrees_dir
            .clone()
            .unwrap_or_else(|| ".worktrees".to_string())
    }

    pub fn branch_prefix(&self) -> String {
        self.defaults
            .branch_prefix
            .clone()
            .unwrap_or_else(|| "wt/".to_string())
    }

    pub fn default_base(&self) -> Option<String> {
        if let Ok(value) = env::var("GW_DEFAULT_BASE") {
            return Some(value);
        }
        self.defaults.base.clone()
    }

    pub fn gc_stale_days(&self) -> i64 {
        self.gc.stale_days.unwrap_or(7)
    }

    pub fn verify_rust(&self) -> String {
        self.verify
            .rust
            .clone()
            .unwrap_or_else(|| "cargo test".to_string())
    }

    pub fn verify_node(&self) -> String {
        self.verify
            .node
            .clone()
            .unwrap_or_else(|| "npm test".to_string())
    }

    pub fn verify_python(&self) -> String {
        self.verify
            .python
            .clone()
            .unwrap_or_else(|| "pytest".to_string())
    }
}

fn merge(base: Config, override_cfg: Config) -> Config {
    Config {
        defaults: Defaults {
            base: override_cfg.defaults.base.or(base.defaults.base),
            worktrees_dir: override_cfg
                .defaults
                .worktrees_dir
                .or(base.defaults.worktrees_dir),
            branch_prefix: override_cfg
                .defaults
                .branch_prefix
                .or(base.defaults.branch_prefix),
        },
        gc: GcConfig {
            stale_days: override_cfg.gc.stale_days.or(base.gc.stale_days),
        },
        verify: VerifyConfig {
            rust: override_cfg.verify.rust.or(base.verify.rust),
            node: override_cfg.verify.node.or(base.verify.node),
            python: override_cfg.verify.python.or(base.verify.python),
        },
    }
}

pub fn gw_home() -> Option<PathBuf> {
    if let Ok(home) = env::var("GW_HOME") {
        return Some(PathBuf::from(home));
    }
    let home = home_dir()?;
    Some(home.join(".gw"))
}

fn global_config_path() -> Option<PathBuf> {
    let home = gw_home()?;
    Some(home.join("config.toml"))
}

fn home_dir() -> Option<PathBuf> {
    if let Ok(home) = env::var("HOME") {
        return Some(PathBuf::from(home));
    }
    if let Ok(profile) = env::var("USERPROFILE") {
        return Some(PathBuf::from(profile));
    }
    None
}
