# TaskFlow

> Industry-grade async task management system built in Rust.
> Features a rich CLI, SQLite persistence with versioned migrations, and an embedded REST API server.

---

## Features

- **Full CLI** with colored output, table rendering, and JSON export
- **SQLite persistence** via `rusqlite` with WAL mode and versioned schema migrations
- **REST API server** with JSON endpoints for all task and project operations
- **Partial ID resolution** — reference tasks by their first 4+ characters
- **Priority & status workflow** — critical / high / medium / low × todo / in_progress / blocked / done / cancelled
- **Filtering** — by status, priority, project, assignee, tag, free-text search, or overdue
- **Project grouping** with optional hex color
- **Tag system** with multi-tag filtering
- **Due date tracking** with overdue highlighting
- **Config file** at `~/.taskflow/config.toml` with env-var overrides
- **Structured logging** via `env_logger`

---

## Architecture

```
src/
├── main.rs              # Entry point, wires config + db + CLI
├── config/mod.rs        # TOML config + env-var overrides
├── models/
│   ├── task.rs          # Task, TaskStatus, TaskPriority, CRUD request types
│   ├── project.rs       # Project domain model
│   └── tag.rs           # Tag aggregate
├── db/
│   ├── mod.rs           # Database handle (Arc<Mutex<Connection>>)
│   ├── migrations.rs    # Versioned schema migrations
│   ├── task_repo.rs     # Full task CRUD + statistics
│   └── project_repo.rs  # Project CRUD
├── cli/
│   ├── mod.rs           # Clap CLI definition + command dispatch
│   ├── commands.rs      # Business logic for each CLI subcommand
│   └── display.rs       # Colored table + detail renderers
└── api/
    ├── mod.rs           # Server entry + tokio spawn_blocking
    ├── router.rs        # URL routing (method + path segments)
    └── handlers.rs      # JSON request/response handlers
```

---

## Installation

```bash
cargo build --release
cp target/release/taskflow /usr/local/bin/
```

---

## CLI Usage

### Tasks

```bash
# Add a task
taskflow add "Refactor authentication module" \
  --priority high \
  --assignee alice \
  --tags "backend,security" \
  --due 2025-12-31

# List all tasks (table view)
taskflow list

# Filter — overdue high-priority tasks assigned to alice
taskflow list --priority high --assignee alice --overdue

# Filter — full-text search, JSON output
taskflow list --search "auth" --format json

# Show detailed view (partial ID OK)
taskflow show a1b2c3d4

# Mark done
taskflow done a1b2c3

# Update fields
taskflow update a1b2c3 --status in_progress --priority critical

# Delete (with confirmation)
taskflow delete a1b2c3

# Delete without prompt
taskflow delete a1b2c3 --yes
```

### Projects

```bash
taskflow project add "Backend Rewrite" --color "#4f46e5"
taskflow project list
taskflow project delete <project-id>
```

### Dashboard

```bash
taskflow stats
```

### REST API Server

```bash
taskflow serve --host 0.0.0.0 --port 8765
```

---

## REST API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/api/v1/tasks` | List tasks (supports query params) |
| POST | `/api/v1/tasks` | Create a task |
| GET | `/api/v1/tasks/:id` | Get a task |
| PUT | `/api/v1/tasks/:id` | Update a task |
| DELETE | `/api/v1/tasks/:id` | Delete a task |
| POST | `/api/v1/tasks/:id/done` | Mark a task done |
| GET | `/api/v1/projects` | List projects |
| POST | `/api/v1/projects` | Create a project |
| DELETE | `/api/v1/projects/:id` | Delete a project |
| GET | `/api/v1/stats` | Aggregate statistics |

### Example requests

```bash
# Create
curl -X POST http://localhost:8765/api/v1/tasks \
  -H 'Content-Type: application/json' \
  -d '{"title":"Deploy v2","priority":"high","tags":["ops"]}'

# List with filter
curl 'http://localhost:8765/api/v1/tasks?status=todo&priority=high'

# Mark done
curl -X POST http://localhost:8765/api/v1/tasks/<id>/done
```

---

## Configuration

`~/.taskflow/config.toml`

```toml
[database]
path = "/home/user/.taskflow/tasks.db"

[server]
host = "127.0.0.1"
port = 8765

[log]
level = "info"
```

### Environment variables

| Variable | Effect |
|----------|--------|
| `TASKFLOW_DB_PATH` | Override database path |
| `TASKFLOW_HOST` | Override server host |
| `TASKFLOW_PORT` | Override server port |
| `TASKFLOW_LOG_LEVEL` | Override log level |
| `RUST_LOG` | Standard env_logger control |

---

## Database Schema

```sql
-- Projects
CREATE TABLE projects (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL UNIQUE,
  description TEXT,
  color       TEXT,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

-- Tasks
CREATE TABLE tasks (
  id           TEXT PRIMARY KEY,
  title        TEXT NOT NULL,
  description  TEXT,
  status       TEXT NOT NULL DEFAULT 'todo',
  priority     TEXT NOT NULL DEFAULT 'medium',
  project_id   TEXT REFERENCES projects(id) ON DELETE SET NULL,
  assignee     TEXT,
  due_date     TEXT,
  completed_at TEXT,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
);

-- Tags (many-to-many)
CREATE TABLE task_tags (
  task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  tag     TEXT NOT NULL,
  PRIMARY KEY (task_id, tag)
);
```

---

## License

MIT
