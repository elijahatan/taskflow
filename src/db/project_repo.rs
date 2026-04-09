use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use uuid::Uuid;

use crate::models::project::{CreateProjectRequest, Project};
use super::Database;

pub struct ProjectRepository<'a> {
    db: &'a Database,
}

impl<'a> ProjectRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn create(&self, req: CreateProjectRequest) -> Result<Project> {
        req.validate()?;

        let project = Project {
            id: Uuid::new_v4().to_string(),
            name: req.name.trim().to_string(),
            description: req.description,
            color: req.color,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO projects (id, name, description, color, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    project.id,
                    project.name,
                    project.description,
                    project.color,
                    project.created_at.to_rfc3339(),
                    project.updated_at.to_rfc3339(),
                ],
            )?;
            Ok(project)
        })
    }

    pub fn list(&self) -> Result<Vec<Project>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, description, color, created_at, updated_at
                   FROM projects ORDER BY name"
            )?;
            let projects = stmt.query_map([], |row| {
                let created_at_str: String = row.get(4)?;
                let updated_at_str: String = row.get(5)?;
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    color: row.get(3)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .map_err(|e| rusqlite::Error::InvalidColumnType(4, e.to_string(), rusqlite::types::Type::Text))?
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                        .map_err(|e| rusqlite::Error::InvalidColumnType(5, e.to_string(), rusqlite::types::Type::Text))?
                        .with_timezone(&Utc),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
            Ok(projects)
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Project>> {
        self.db.with_conn(|conn| {
            let result = conn.query_row(
                "SELECT id, name, description, color, created_at, updated_at
                   FROM projects WHERE id = ?1",
                params![id],
                |row| {
                    let created_at_str: String = row.get(4)?;
                    let updated_at_str: String = row.get(5)?;
                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        color: row.get(3)?,
                        created_at: DateTime::parse_from_rfc3339(&created_at_str)
                            .map_err(|e| rusqlite::Error::InvalidColumnType(4, e.to_string(), rusqlite::types::Type::Text))?
                            .with_timezone(&Utc),
                        updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                            .map_err(|e| rusqlite::Error::InvalidColumnType(5, e.to_string(), rusqlite::types::Type::Text))?
                            .with_timezone(&Utc),
                    })
                },
            );
            match result {
                Ok(p) => Ok(Some(p)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        self.db.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM projects WHERE id=?1", params![id])?;
            Ok(rows > 0)
        })
    }
}
