use anyhow::Result;
use log::info;
use rusqlite::Connection;

pub fn run_all(conn: &Connection) -> Result<()> {
    bootstrap_version_table(conn)?;

    let applied = get_applied_versions(conn)?;

    for (version, sql) in MIGRATIONS.iter() {
        if !applied.contains(version) {
            info!("Applying migration v{}", version);
            conn.execute_batch(sql)?;
            mark_applied(conn, version)?;
        }
    }

    Ok(())
}

fn bootstrap_version_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_versions (
            version   INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )?;
    Ok(())
}

fn get_applied_versions(conn: &Connection) -> Result<Vec<u32>> {
    let mut stmt = conn.prepare("SELECT version FROM schema_versions ORDER BY version")?;
    let versions = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<u32>, _>>()?;
    Ok(versions)
}

fn mark_applied(conn: &Connection, version: &u32) -> Result<()> {
    conn.execute(
        "INSERT INTO schema_versions (version) VALUES (?1)",
        [version],
    )?;
    Ok(())
}

static MIGRATIONS: &[(u32, &str)] = &[
    (1, MIGRATION_001_INITIAL),
    (2, MIGRATION_002_TAGS),
    (3, MIGRATION_003_INDEXES),
    (4, MIGRATION_004_DEPENDENCIES),
];

const MIGRATION_001_INITIAL: &str = "
BEGIN;

CREATE TABLE IF NOT EXISTS projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT,
    color       TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT,
    status      TEXT NOT NULL DEFAULT 'todo',
    priority    TEXT NOT NULL DEFAULT 'medium',
    project_id  TEXT REFERENCES projects(id) ON DELETE SET NULL,
    assignee    TEXT,
    due_date    TEXT,
    completed_at TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

COMMIT;
";

const MIGRATION_002_TAGS: &str = "
BEGIN;

CREATE TABLE IF NOT EXISTS task_tags (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    tag     TEXT NOT NULL,
    PRIMARY KEY (task_id, tag)
);

COMMIT;
";

const MIGRATION_003_INDEXES: &str = "
BEGIN;

CREATE INDEX IF NOT EXISTS idx_tasks_status    ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_priority  ON tasks(priority);
CREATE INDEX IF NOT EXISTS idx_tasks_project   ON tasks(project_id);
CREATE INDEX IF NOT EXISTS idx_tasks_assignee  ON tasks(assignee);
CREATE INDEX IF NOT EXISTS idx_tasks_due_date  ON tasks(due_date);
CREATE INDEX IF NOT EXISTS idx_task_tags_tag   ON task_tags(tag);

COMMIT;
";

const MIGRATION_004_DEPENDENCIES: &str = "
BEGIN;

CREATE TABLE IF NOT EXISTS task_dependencies (
    task_id            TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, depends_on_task_id),
    CHECK (task_id <> depends_on_task_id)
);

CREATE INDEX IF NOT EXISTS idx_task_dependencies_task_id
    ON task_dependencies(task_id);
CREATE INDEX IF NOT EXISTS idx_task_dependencies_depends_on
    ON task_dependencies(depends_on_task_id);

COMMIT;
";
