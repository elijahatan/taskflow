pub mod migrations;
pub mod project_repo;
pub mod task_repo;

use anyhow::Result;
use log::info;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        conn.execute_batch("PRAGMA busy_timeout=5000;")?;

        let db = Database {
            conn: Arc::new(Mutex::new(conn)),
        };

        db.run_migrations()?;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        let db = Database {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.run_migrations()?;
        Ok(db)
    }

    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("DB mutex poisoned"))?;
        f(&conn)
    }

    fn run_migrations(&self) -> Result<()> {
        self.with_conn(|conn| {
            migrations::run_all(conn)?;
            info!("Database migrations completed");
            Ok(())
        })
    }

    pub fn stats(&self) -> Result<DbStats> {
        self.with_conn(|conn| {
            let task_count: u32 =
                conn.query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))?;
            let project_count: u32 =
                conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))?;
            let done_count: u32 = conn.query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'done'",
                [],
                |row| row.get(0),
            )?;
            Ok(DbStats {
                task_count,
                project_count,
                done_count,
            })
        })
    }
}

#[derive(Debug)]
pub struct DbStats {
    pub task_count: u32,
    pub project_count: u32,
    pub done_count: u32,
}
