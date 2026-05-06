use anyhow::Result;
use chrono::{DateTime, Utc};
use log::debug;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::models::task::{
    CreateTaskRequest, Task, TaskActivity, TaskFilter, TaskPriority, TaskRecurrence, TaskStatus,
    UpdateTaskRequest,
};

use super::Database;

pub struct TaskRepository<'a> {
    db: &'a Database,
}

impl<'a> TaskRepository<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn create(&self, req: CreateTaskRequest) -> Result<Task> {
        req.validate()?;

        let task = Task {
            id: Uuid::new_v4().to_string(),
            title: req.title.trim().to_string(),
            description: req.description,
            status: TaskStatus::Todo,
            priority: req.priority.unwrap_or_default(),
            project_id: req.project_id,
            assignee: req.assignee,
            tags: req.tags.unwrap_or_default(),
            blocked_by: Vec::new(),
            blocks: Vec::new(),
            activities: Vec::new(),
            recurrence: req.recurrence,
            due_date: req.due_date,
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO tasks (id, title, description, status, priority, project_id,
                  assignee, recurrence, due_date, completed_at, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    task.id,
                    task.title,
                    task.description,
                    task.status.as_str(),
                    task.priority.as_str(),
                    task.project_id,
                    task.assignee,
                    task.recurrence.as_ref().map(|r| r.as_str().to_string()),
                    task.due_date.map(|d| d.to_rfc3339()),
                    task.completed_at.map(|d| d.to_rfc3339()),
                    task.created_at.to_rfc3339(),
                    task.updated_at.to_rfc3339(),
                ],
            )?;

            for tag in &task.tags {
                conn.execute(
                    "INSERT OR IGNORE INTO task_tags (task_id, tag) VALUES (?1, ?2)",
                    params![task.id, tag],
                )?;
            }
            append_activity(conn, &task.id, "created", None)?;

            debug!("Created task {}: {}", task.id, task.title);
            Ok(task)
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Task>> {
        self.db.with_conn(|conn| {
            let result = conn.query_row(
                "SELECT id, title, description, status, priority, project_id,
                        assignee, recurrence, due_date, completed_at, created_at, updated_at
                   FROM tasks WHERE id = ?1",
                params![id],
                row_to_task,
            );

            match result {
                Ok(mut task) => {
                    task.tags = get_tags_for_task(conn, &task.id)?;
                    task.blocked_by = get_dependency_ids(conn, &task.id)?;
                    task.blocks = get_dependent_ids(conn, &task.id)?;
                    task.activities = get_activity_for_task(conn, &task.id)?;
                    Ok(Some(task))
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }

    pub fn list(&self, filter: &TaskFilter) -> Result<Vec<Task>> {
        self.db.with_conn(|conn| {
            let mut where_clauses: Vec<String> = Vec::new();
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(status) = &filter.status {
                where_clauses.push(format!("t.status = ?{}", params_vec.len() + 1));
                params_vec.push(Box::new(status.as_str().to_string()));
            }
            if let Some(priority) = &filter.priority {
                where_clauses.push(format!("t.priority = ?{}", params_vec.len() + 1));
                params_vec.push(Box::new(priority.as_str().to_string()));
            }
            if let Some(project_id) = &filter.project_id {
                where_clauses.push(format!("t.project_id = ?{}", params_vec.len() + 1));
                params_vec.push(Box::new(project_id.clone()));
            }
            if let Some(assignee) = &filter.assignee {
                where_clauses.push(format!("t.assignee = ?{}", params_vec.len() + 1));
                params_vec.push(Box::new(assignee.clone()));
            }
            if let Some(search) = &filter.search {
                where_clauses.push(format!(
                    "(t.title LIKE ?{0} OR t.description LIKE ?{0})",
                    params_vec.len() + 1
                ));
                params_vec.push(Box::new(format!("%{}%", search)));
            }
            if filter.overdue_only {
                where_clauses.push(
                    "t.due_date < datetime('now') AND t.status NOT IN ('done','cancelled')".into(),
                );
            }
            if let Some(tag) = &filter.tag {
                where_clauses.push(format!(
                    "EXISTS (SELECT 1 FROM task_tags tt WHERE tt.task_id = t.id AND tt.tag = ?{})",
                    params_vec.len() + 1
                ));
                params_vec.push(Box::new(tag.clone()));
            }

            let where_sql = if where_clauses.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", where_clauses.join(" AND "))
            };

            let limit_sql = match filter.limit {
                Some(l) => format!("LIMIT {}", l),
                None => String::new(),
            };
            let offset_sql = match filter.offset {
                Some(o) => format!("OFFSET {}", o),
                None => String::new(),
            };

            let query = format!(
                "SELECT t.id, t.title, t.description, t.status, t.priority, t.project_id,
                        t.assignee, t.recurrence, t.due_date, t.completed_at, t.created_at, t.updated_at
                   FROM tasks t
                   {where_sql}
                   ORDER BY
                     CASE t.priority
                       WHEN 'critical' THEN 0
                       WHEN 'high'     THEN 1
                       WHEN 'medium'   THEN 2
                       WHEN 'low'      THEN 3
                       ELSE 4
                     END,
                     t.created_at ASC
                   {limit_sql} {offset_sql}"
            );

            let mut stmt = conn.prepare(&query)?;
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();

            let tasks = stmt
                .query_map(param_refs.as_slice(), row_to_task)?
                .collect::<Result<Vec<Task>, _>>()?;

            // Attach tags to each task
            let mut result = Vec::with_capacity(tasks.len());
            for mut task in tasks {
                task.tags = get_tags_for_task(conn, &task.id)?;
                task.blocked_by = get_dependency_ids(conn, &task.id)?;
                task.blocks = get_dependent_ids(conn, &task.id)?;
                task.activities = Vec::new();
                result.push(task);
            }

            Ok(result)
        })
    }

    pub fn update(&self, id: &str, req: UpdateTaskRequest) -> Result<Option<Task>> {
        // Verify the task exists first
        if self.get_by_id(id)?.is_none() {
            return Ok(None);
        }

        if matches!(req.status, Some(TaskStatus::Done)) && self.has_unfinished_dependencies(id)? {
            return Err(anyhow::anyhow!(
                "Cannot mark task as done while it still has unfinished dependencies"
            ));
        }

        self.db.with_conn(|conn| {
            let existing = get_task_by_id(conn, id)?
                .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", id))?;
            let now = Utc::now().to_rfc3339();
            let mut changes = Vec::new();

            if let Some(title) = &req.title {
                conn.execute(
                    "UPDATE tasks SET title=?1, updated_at=?2 WHERE id=?3",
                    params![title, now, id],
                )?;
                if existing.title != *title {
                    changes.push(format!("title: '{}' -> '{}'", existing.title, title));
                }
            }
            if let Some(desc) = &req.description {
                conn.execute(
                    "UPDATE tasks SET description=?1, updated_at=?2 WHERE id=?3",
                    params![desc, now, id],
                )?;
                if existing.description != Some(desc.clone()) {
                    changes.push("description updated".to_string());
                }
            }
            if let Some(status) = &req.status {
                let completed_at = if *status == TaskStatus::Done {
                    Some(now.clone())
                } else {
                    None
                };
                conn.execute(
                    "UPDATE tasks SET status=?1, completed_at=?2, updated_at=?3 WHERE id=?4",
                    params![status.as_str(), completed_at, now, id],
                )?;
                if existing.status != *status {
                    changes.push(format!("status: {} -> {}", existing.status, status));
                }
            }
            if let Some(priority) = &req.priority {
                conn.execute(
                    "UPDATE tasks SET priority=?1, updated_at=?2 WHERE id=?3",
                    params![priority.as_str(), now, id],
                )?;
                if existing.priority != *priority {
                    changes.push(format!("priority: {} -> {}", existing.priority, priority));
                }
            }
            if let Some(assignee) = &req.assignee {
                conn.execute(
                    "UPDATE tasks SET assignee=?1, updated_at=?2 WHERE id=?3",
                    params![assignee, now, id],
                )?;
                if existing.assignee != Some(assignee.clone()) {
                    changes.push(format!("assignee set to {}", assignee));
                }
            }
            if let Some(project_id) = &req.project_id {
                conn.execute(
                    "UPDATE tasks SET project_id=?1, updated_at=?2 WHERE id=?3",
                    params![project_id, now, id],
                )?;
                if existing.project_id != Some(project_id.clone()) {
                    changes.push(format!("project set to {}", project_id));
                }
            }
            if let Some(recurrence) = &req.recurrence {
                conn.execute(
                    "UPDATE tasks SET recurrence=?1, updated_at=?2 WHERE id=?3",
                    params![recurrence.as_ref().map(|r| r.as_str().to_string()), now, id],
                )?;
                if existing.recurrence != *recurrence {
                    let value = recurrence
                        .as_ref()
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "none".to_string());
                    changes.push(format!("recurrence set to {}", value));
                }
            }
            if let Some(due_date) = &req.due_date {
                conn.execute(
                    "UPDATE tasks SET due_date=?1, updated_at=?2 WHERE id=?3",
                    params![due_date.to_rfc3339(), now, id],
                )?;
                if existing.due_date != Some(*due_date) {
                    changes.push(format!("due date set to {}", due_date.format("%Y-%m-%d")));
                }
            }
            if let Some(tags) = &req.tags {
                conn.execute("DELETE FROM task_tags WHERE task_id=?1", params![id])?;
                for tag in tags {
                    conn.execute(
                        "INSERT OR IGNORE INTO task_tags (task_id, tag) VALUES (?1,?2)",
                        params![id, tag],
                    )?;
                }
                if existing.tags != *tags {
                    changes.push(format!("tags set to {}", tags.join(", ")));
                }
            }

            if !changes.is_empty() {
                append_activity(conn, id, "updated", Some(changes.join("; ")))?;
            }
            sync_task_status_with_dependencies(conn, id)?;
            sync_dependents_for_task(conn, id)?;
            if matches!(req.status, Some(TaskStatus::Done)) && existing.status != TaskStatus::Done {
                append_activity(conn, id, "completed", None)?;
                create_next_recurring_task(conn, &existing)?;
            }

            Ok(())
        })?;

        self.get_by_id(id)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        self.db.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM tasks WHERE id=?1", params![id])?;
            Ok(rows > 0)
        })
    }

    pub fn statistics(&self) -> Result<TaskStatistics> {
        self.db.with_conn(|conn| {
            let total: u32 = conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))?;
            let todo: u32 = conn.query_row("SELECT COUNT(*) FROM tasks WHERE status='todo'", [], |r| r.get(0))?;
            let in_progress: u32 = conn.query_row("SELECT COUNT(*) FROM tasks WHERE status='in_progress'", [], |r| r.get(0))?;
            let done: u32 = conn.query_row("SELECT COUNT(*) FROM tasks WHERE status='done'", [], |r| r.get(0))?;
            let overdue: u32 = conn.query_row(
                "SELECT COUNT(*) FROM tasks WHERE due_date < datetime('now') AND status NOT IN ('done','cancelled')",
                [], |r| r.get(0)
            )?;
            let critical: u32 = conn.query_row(
                "SELECT COUNT(*) FROM tasks WHERE priority='critical' AND status NOT IN ('done','cancelled')",
                [], |r| r.get(0)
            )?;
            Ok(TaskStatistics { total, todo, in_progress, done, overdue, critical })
        })
    }

    pub fn add_dependency(&self, task_id: &str, depends_on_task_id: &str) -> Result<Task> {
        if task_id == depends_on_task_id {
            return Err(anyhow::anyhow!("A task cannot depend on itself"));
        }
        if self.get_by_id(task_id)?.is_none() {
            return Err(anyhow::anyhow!("Task '{}' not found", task_id));
        }
        if self.get_by_id(depends_on_task_id)?.is_none() {
            return Err(anyhow::anyhow!("Task '{}' not found", depends_on_task_id));
        }

        self.db.with_conn(|conn| {
            if dependency_exists(conn, task_id, depends_on_task_id)? {
                return Err(anyhow::anyhow!("Dependency already exists"));
            }
            if creates_cycle(conn, task_id, depends_on_task_id)? {
                return Err(anyhow::anyhow!(
                    "Cannot add dependency because it would create a cycle"
                ));
            }

            conn.execute(
                "INSERT INTO task_dependencies (task_id, depends_on_task_id) VALUES (?1, ?2)",
                params![task_id, depends_on_task_id],
            )?;

            append_activity(
                conn,
                task_id,
                "dependency_added",
                Some(format!("blocked by {}", depends_on_task_id)),
            )?;
            sync_task_status_with_dependencies(conn, task_id)?;
            Ok(())
        })?;

        self.get_by_id(task_id)?
            .ok_or_else(|| anyhow::anyhow!("Task '{}' not found after update", task_id))
    }

    pub fn remove_dependency(&self, task_id: &str, depends_on_task_id: &str) -> Result<Task> {
        if self.get_by_id(task_id)?.is_none() {
            return Err(anyhow::anyhow!("Task '{}' not found", task_id));
        }

        self.db.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM task_dependencies WHERE task_id=?1 AND depends_on_task_id=?2",
                params![task_id, depends_on_task_id],
            )?;

            if rows == 0 {
                return Err(anyhow::anyhow!("Dependency does not exist"));
            }

            append_activity(
                conn,
                task_id,
                "dependency_removed",
                Some(format!("unblocked from {}", depends_on_task_id)),
            )?;
            sync_task_status_with_dependencies(conn, task_id)?;
            Ok(())
        })?;

        self.get_by_id(task_id)?
            .ok_or_else(|| anyhow::anyhow!("Task '{}' not found after update", task_id))
    }

    pub fn has_unfinished_dependencies(&self, task_id: &str) -> Result<bool> {
        self.db
            .with_conn(|conn| has_unfinished_dependencies(conn, task_id))
    }

    pub fn activity(&self, task_id: &str) -> Result<Vec<TaskActivity>> {
        self.db
            .with_conn(|conn| get_activity_for_task(conn, task_id))
    }
}

fn row_to_task(row: &rusqlite::Row) -> rusqlite::Result<Task> {
    let status_str: String = row.get(3)?;
    let priority_str: String = row.get(4)?;
    let recurrence_str: Option<String> = row.get(7)?;
    let due_date_str: Option<String> = row.get(8)?;
    let completed_at_str: Option<String> = row.get(9)?;
    let created_at_str: String = row.get(10)?;
    let updated_at_str: String = row.get(11)?;

    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: TaskStatus::from_str(&status_str).map_err(|e| {
            rusqlite::Error::InvalidColumnType(3, e.to_string(), rusqlite::types::Type::Text)
        })?,
        priority: TaskPriority::from_str(&priority_str).map_err(|e| {
            rusqlite::Error::InvalidColumnType(4, e.to_string(), rusqlite::types::Type::Text)
        })?,
        project_id: row.get(5)?,
        assignee: row.get(6)?,
        tags: Vec::new(),       // filled in separately
        blocked_by: Vec::new(), // filled in separately
        blocks: Vec::new(),     // filled in separately
        activities: Vec::new(), // filled in separately
        recurrence: recurrence_str
            .map(|s| TaskRecurrence::from_str(&s))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::InvalidColumnType(7, e.to_string(), rusqlite::types::Type::Text)
            })?,
        due_date: due_date_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|d| d.with_timezone(&Utc))
        }),
        completed_at: completed_at_str.and_then(|s| {
            DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|d| d.with_timezone(&Utc))
        }),
        created_at: DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                rusqlite::Error::InvalidColumnType(10, e.to_string(), rusqlite::types::Type::Text)
            })?
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| {
                rusqlite::Error::InvalidColumnType(11, e.to_string(), rusqlite::types::Type::Text)
            })?
            .with_timezone(&Utc),
    })
}

fn get_task_by_id(conn: &Connection, id: &str) -> Result<Option<Task>> {
    let result = conn.query_row(
        "SELECT id, title, description, status, priority, project_id,
                assignee, recurrence, due_date, completed_at, created_at, updated_at
           FROM tasks WHERE id = ?1",
        params![id],
        row_to_task,
    );

    match result {
        Ok(mut task) => {
            task.tags = get_tags_for_task(conn, &task.id)?;
            task.blocked_by = get_dependency_ids(conn, &task.id)?;
            task.blocks = get_dependent_ids(conn, &task.id)?;
            task.activities = get_activity_for_task(conn, &task.id)?;
            Ok(Some(task))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn get_tags_for_task(conn: &Connection, task_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT tag FROM task_tags WHERE task_id=?1 ORDER BY tag")?;
    let tags = stmt
        .query_map(params![task_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(tags)
}

fn get_dependency_ids(conn: &Connection, task_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT depends_on_task_id
           FROM task_dependencies
          WHERE task_id=?1
          ORDER BY depends_on_task_id",
    )?;
    let ids = stmt
        .query_map(params![task_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}

fn get_activity_for_task(conn: &Connection, task_id: &str) -> Result<Vec<TaskActivity>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, action, details, created_at
           FROM task_activity
          WHERE task_id=?1
          ORDER BY created_at DESC",
    )?;
    let items = stmt
        .query_map(params![task_id], |row| {
            let created_at_str: String = row.get(4)?;
            Ok(TaskActivity {
                id: row.get(0)?,
                task_id: row.get(1)?,
                action: row.get(2)?,
                details: row.get(3)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map_err(|e| {
                        rusqlite::Error::InvalidColumnType(
                            4,
                            e.to_string(),
                            rusqlite::types::Type::Text,
                        )
                    })?
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<TaskActivity>, _>>()?;
    Ok(items)
}

fn append_activity(
    conn: &Connection,
    task_id: &str,
    action: &str,
    details: Option<String>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO task_activity (id, task_id, action, details, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            Uuid::new_v4().to_string(),
            task_id,
            action,
            details,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}

fn get_dependent_ids(conn: &Connection, task_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT task_id
           FROM task_dependencies
          WHERE depends_on_task_id=?1
          ORDER BY task_id",
    )?;
    let ids = stmt
        .query_map(params![task_id], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}

fn dependency_exists(conn: &Connection, task_id: &str, depends_on_task_id: &str) -> Result<bool> {
    let exists: u32 = conn.query_row(
        "SELECT COUNT(*) FROM task_dependencies WHERE task_id=?1 AND depends_on_task_id=?2",
        params![task_id, depends_on_task_id],
        |row| row.get(0),
    )?;
    Ok(exists > 0)
}

fn has_unfinished_dependencies(conn: &Connection, task_id: &str) -> Result<bool> {
    let count: u32 = conn.query_row(
        "SELECT COUNT(*)
           FROM task_dependencies td
           JOIN tasks t ON t.id = td.depends_on_task_id
          WHERE td.task_id = ?1
            AND t.status NOT IN ('done', 'cancelled')",
        params![task_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn creates_cycle(conn: &Connection, task_id: &str, depends_on_task_id: &str) -> Result<bool> {
    fn visit(conn: &Connection, current: &str, target: &str) -> Result<bool> {
        let mut stmt =
            conn.prepare("SELECT depends_on_task_id FROM task_dependencies WHERE task_id=?1")?;
        let next_ids = stmt
            .query_map(params![current], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<String>, _>>()?;

        for next in next_ids {
            if next == target || visit(conn, &next, target)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    visit(conn, depends_on_task_id, task_id)
}

fn sync_task_status_with_dependencies(conn: &Connection, task_id: &str) -> Result<()> {
    let status: String = conn.query_row(
        "SELECT status FROM tasks WHERE id=?1",
        params![task_id],
        |row| row.get(0),
    )?;

    if matches!(status.as_str(), "done" | "cancelled") {
        return Ok(());
    }

    let now = Utc::now().to_rfc3339();
    if has_unfinished_dependencies(conn, task_id)? {
        if status != "blocked" {
            conn.execute(
                "UPDATE tasks SET status='blocked', updated_at=?1 WHERE id=?2",
                params![now, task_id],
            )?;
            append_activity(
                conn,
                task_id,
                "blocked",
                Some("waiting on dependencies".to_string()),
            )?;
        }
    } else if status == "blocked" {
        conn.execute(
            "UPDATE tasks SET status='todo', updated_at=?1 WHERE id=?2",
            params![now, task_id],
        )?;
        append_activity(
            conn,
            task_id,
            "unblocked",
            Some("dependencies resolved".to_string()),
        )?;
    }

    Ok(())
}

fn sync_dependents_for_task(conn: &Connection, task_id: &str) -> Result<()> {
    for dependent_id in get_dependent_ids(conn, task_id)? {
        sync_task_status_with_dependencies(conn, &dependent_id)?;
    }
    Ok(())
}

fn create_next_recurring_task(conn: &Connection, task: &Task) -> Result<()> {
    let Some(recurrence) = &task.recurrence else {
        return Ok(());
    };

    let next_due_date = task
        .due_date
        .and_then(|due_date| recurrence.next_due_date(due_date));
    let now = Utc::now();
    let next_task_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO tasks (id, title, description, status, priority, project_id,
          assignee, recurrence, due_date, completed_at, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            next_task_id,
            &task.title,
            task.description.as_deref(),
            TaskStatus::Todo.as_str(),
            task.priority.as_str(),
            task.project_id.as_deref(),
            task.assignee.as_deref(),
            recurrence.as_str(),
            next_due_date.map(|d| d.to_rfc3339()),
            Option::<String>::None,
            now.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;

    for tag in &task.tags {
        conn.execute(
            "INSERT OR IGNORE INTO task_tags (task_id, tag) VALUES (?1, ?2)",
            params![next_task_id, tag],
        )?;
    }

    append_activity(
        conn,
        task.id.as_str(),
        "recurrence_spawned",
        Some(format!("created next occurrence {}", next_task_id)),
    )?;
    append_activity(
        conn,
        &next_task_id,
        "created",
        Some(format!("spawned from recurring task {}", task.id)),
    )?;

    Ok(())
}

#[derive(Debug)]
pub struct TaskStatistics {
    pub total: u32,
    pub todo: u32,
    pub in_progress: u32,
    pub done: u32,
    pub overdue: u32,
    pub critical: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone};

    fn end_of_day_utc(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
        Utc.from_utc_datetime(&date.and_hms_opt(23, 59, 59).unwrap())
    }

    #[test]
    fn completing_recurring_task_creates_next_occurrence() {
        let db = Database::open_in_memory().unwrap();
        let repo = TaskRepository::new(&db);

        let original = repo
            .create(CreateTaskRequest {
                title: "Pay rent".to_string(),
                description: Some("Monthly recurring bill".to_string()),
                priority: Some(TaskPriority::High),
                project_id: None,
                assignee: Some("alex".to_string()),
                tags: Some(vec!["finance".to_string()]),
                recurrence: Some(TaskRecurrence::Monthly),
                due_date: Some(end_of_day_utc(2026, 5, 31)),
            })
            .unwrap();

        let completed = repo
            .update(
                &original.id,
                UpdateTaskRequest {
                    status: Some(TaskStatus::Done),
                    ..Default::default()
                },
            )
            .unwrap()
            .unwrap();

        assert_eq!(completed.status, TaskStatus::Done);

        let tasks = repo.list(&TaskFilter::default()).unwrap();
        assert_eq!(tasks.len(), 2);

        let successor = tasks
            .into_iter()
            .find(|task| task.id != original.id)
            .unwrap();

        assert_eq!(successor.status, TaskStatus::Todo);
        assert_eq!(successor.title, original.title);
        assert_eq!(successor.tags, vec!["finance".to_string()]);
        assert_eq!(successor.recurrence, Some(TaskRecurrence::Monthly));
        assert_eq!(successor.assignee.as_deref(), Some("alex"));
        assert_eq!(successor.due_date, Some(end_of_day_utc(2026, 6, 30)));
    }

    #[test]
    fn task_activity_history_records_key_events() {
        let db = Database::open_in_memory().unwrap();
        let repo = TaskRepository::new(&db);

        let blocker = repo
            .create(CreateTaskRequest {
                title: "Prepare data".to_string(),
                description: None,
                priority: None,
                project_id: None,
                assignee: None,
                tags: None,
                recurrence: None,
                due_date: None,
            })
            .unwrap();

        let task = repo
            .create(CreateTaskRequest {
                title: "Publish report".to_string(),
                description: None,
                priority: None,
                project_id: None,
                assignee: None,
                tags: Some(vec!["reporting".to_string()]),
                recurrence: None,
                due_date: None,
            })
            .unwrap();

        repo.add_dependency(&task.id, &blocker.id).unwrap();
        repo.update(
            &task.id,
            UpdateTaskRequest {
                priority: Some(TaskPriority::High),
                ..Default::default()
            },
        )
        .unwrap();
        repo.update(
            &blocker.id,
            UpdateTaskRequest {
                status: Some(TaskStatus::Done),
                ..Default::default()
            },
        )
        .unwrap();

        let activity = repo.activity(&task.id).unwrap();
        let actions = activity
            .iter()
            .map(|item| item.action.as_str())
            .collect::<Vec<_>>();

        assert!(actions.contains(&"created"));
        assert!(actions.contains(&"dependency_added"));
        assert!(actions.contains(&"blocked"));
        assert!(actions.contains(&"updated"));
        assert!(actions.contains(&"unblocked"));
    }
}
