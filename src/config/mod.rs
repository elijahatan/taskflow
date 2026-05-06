use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::models::task::{TaskFilter, TaskPriority, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub log: LogConfig,
    #[serde(default)]
    pub smart_filters: BTreeMap<String, SavedFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavedFilter {
    pub status: Option<String>,
    pub priority: Option<String>,
    pub project_id: Option<String>,
    pub assignee: Option<String>,
    pub tag: Option<String>,
    pub search: Option<String>,
    #[serde(default)]
    pub overdue_only: bool,
    pub limit: Option<u32>,
}

impl SavedFilter {
    pub fn to_task_filter(&self) -> Result<TaskFilter> {
        Ok(TaskFilter {
            status: self
                .status
                .as_deref()
                .map(TaskStatus::from_str)
                .transpose()?,
            priority: self
                .priority
                .as_deref()
                .map(TaskPriority::from_str)
                .transpose()?,
            project_id: self.project_id.clone(),
            assignee: self.assignee.clone(),
            tag: self.tag.clone(),
            search: self.search.clone(),
            overdue_only: self.overdue_only,
            limit: self.limit,
            offset: None,
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            database: DatabaseConfig {
                path: default_db_path(),
            },
            server: ServerConfig {
                host: "127.0.0.1".into(),
                port: 8765,
            },
            log: LogConfig {
                level: "info".into(),
            },
            smart_filters: BTreeMap::new(),
        }
    }
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let config: Config =
            toml::from_str(&content).map_err(|e| anyhow::anyhow!("Config parse error: {}", e))?;
        Ok(config)
    }

    pub fn load() -> Self {
        let config_path = default_config_path();
        if config_path.exists() {
            match Self::load_from_file(&config_path) {
                Ok(cfg) => {
                    log::info!("Loaded config from {:?}", config_path);
                    return cfg;
                }
                Err(e) => {
                    log::warn!("Failed to load config: {}. Using defaults.", e);
                }
            }
        }
        Self::default()
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let config_path = default_config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Config serialize error: {}", e))?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(db_path) = std::env::var("TASKFLOW_DB_PATH") {
            self.database.path = PathBuf::from(db_path);
        }
        if let Ok(host) = std::env::var("TASKFLOW_HOST") {
            self.server.host = host;
        }
        if let Ok(port) = std::env::var("TASKFLOW_PORT") {
            if let Ok(p) = port.parse::<u16>() {
                self.server.port = p;
            }
        }
        if let Ok(level) = std::env::var("TASKFLOW_LOG_LEVEL") {
            self.log.level = level;
        }
        if let Ok(level) = std::env::var("RUST_LOG") {
            self.log.level = level;
        }
    }

    pub fn get_saved_filter(&self, name: &str) -> Option<&SavedFilter> {
        self.smart_filters.get(name)
    }

    pub fn upsert_saved_filter(&mut self, name: String, filter: SavedFilter) {
        self.smart_filters.insert(name, filter);
    }

    pub fn remove_saved_filter(&mut self, name: &str) -> bool {
        self.smart_filters.remove(name).is_some()
    }
}

fn default_config_path() -> PathBuf {
    dirs_next().join("config.toml")
}

pub fn default_db_path() -> PathBuf {
    dirs_next().join("tasks.db")
}

fn dirs_next() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".taskflow")
}
