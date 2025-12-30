use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct MetaStore {
    path: PathBuf,
    data: MetaData,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetaData {
    #[serde(default)]
    pub worktrees: HashMap<String, WorktreeMeta>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorktreeMeta {
    pub created_at: Option<String>,
    pub created_by: Option<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub last_activity_at: Option<String>,
}

impl MetaStore {
    pub fn new(repo_root: &Path) -> anyhow::Result<Self> {
        let dir = repo_root.join(".gw");
        fs::create_dir_all(&dir)?;
        let path = dir.join("meta.json");
        let data = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            MetaData::default()
        };
        Ok(Self { path, data })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let data = serde_json::to_string_pretty(&self.data)?;
        fs::write(&self.path, data)?;
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&WorktreeMeta> {
        self.data.worktrees.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut WorktreeMeta> {
        self.data.worktrees.get_mut(name)
    }

    pub fn ensure(&mut self, name: &str) -> &mut WorktreeMeta {
        self.data.worktrees.entry(name.to_string()).or_default()
    }

    pub fn set_created(&mut self, name: &str) {
        let meta = self.ensure(name);
        if meta.created_at.is_none() {
            meta.created_at = Some(now());
        }
        if meta.created_by.is_none() {
            meta.created_by = Some(created_by());
        }
    }

    pub fn set_last_activity(&mut self, name: &str) {
        let meta = self.ensure(name);
        meta.last_activity_at = Some(now());
    }

    pub fn add_note(&mut self, name: &str, text: String) {
        let meta = self.ensure(name);
        meta.notes.push(text);
    }

    pub fn remove(&mut self, name: &str) {
        self.data.worktrees.remove(name);
    }

    pub fn all(&self) -> &HashMap<String, WorktreeMeta> {
        &self.data.worktrees
    }
}

fn now() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.to_rfc3339()
}

fn created_by() -> String {
    let user = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_else(|_| "unknown".to_string());
    let host = std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")).unwrap_or_else(|_| "unknown".to_string());
    format!("{}@{}", user, host)
}
