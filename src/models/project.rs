use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
}

impl CreateProjectRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.name.trim().is_empty() {
            return Err(anyhow::anyhow!("Project name cannot be empty"));
        }
        if self.name.len() > 100 {
            return Err(anyhow::anyhow!("Project name must be 100 characters or fewer"));
        }
        if let Some(color) = &self.color {
            if !color.starts_with('#') || color.len() != 7 {
                return Err(anyhow::anyhow!("Color must be a valid hex color like #ff5733"));
            }
        }
        Ok(())
    }
}
