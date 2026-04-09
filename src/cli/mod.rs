pub mod commands;
pub mod display;

use anyhow::{anyhow, Result};
use pico_args::Arguments;

use crate::db::Database;

const HELP: &str = "\
taskflow 1.0.0 — Fast, ergonomic task management

USAGE:
  taskflow [OPTIONS] <COMMAND> [ARGS]

OPTIONS:
  --db <PATH>           Override database path
  --log-level <LEVEL>   Log level  [default: warn]
  -h, --help            Print help

COMMANDS:
  add       Add a new task
  list      List tasks  (alias: ls)
  show      Show task detail
  done      Mark a task as done
  update    Update task fields
  delete    Delete a task  (alias: rm)
  project   Manage projects (sub: add, list, delete)
  stats     Show dashboard statistics
  serve     Start the REST API server
";

pub struct GlobalOpts {
    pub db: Option<String>,
    pub log_level: String,
}

pub enum Command {
    Add {
        title: String,
        description: Option<String>,
        priority: String,
        project: Option<String>,
        assignee: Option<String>,
        tags: Option<String>,
        due: Option<String>,
    },
    List {
        status: Option<String>,
        priority: Option<String>,
        project: Option<String>,
        assignee: Option<String>,
        tag: Option<String>,
        search: Option<String>,
        overdue: bool,
        format: String,
        limit: Option<u32>,
    },
    Show { id: String },
    Done { id: String },
    Update {
        id: String,
        title: Option<String>,
        status: Option<String>,
        priority: Option<String>,
        assignee: Option<String>,
        tags: Option<String>,
        due: Option<String>,
    },
    Delete { id: String, yes: bool },
    ProjectAdd { name: String, description: Option<String>, color: Option<String> },
    ProjectList,
    ProjectDelete { id: String, yes: bool },
    Stats,
    Serve { host: String, port: u16 },
}

pub fn parse_args() -> Result<(GlobalOpts, Command)> {
    let mut args = Arguments::from_env();

    if args.contains(["-h", "--help"]) && std::env::args().len() <= 2 {
        print!("{}", HELP);
        std::process::exit(0);
    }

    let global = GlobalOpts {
        db: args.opt_value_from_str("--db")?,
        log_level: args
            .opt_value_from_str("--log-level")?
            .unwrap_or_else(|| "warn".to_string()),
    };

    let subcommand = args.subcommand()?.unwrap_or_default();

    let cmd = match subcommand.as_str() {
        "add" => {
            let title: String = args
                .free_from_str()
                .map_err(|_| anyhow!("add requires a title. Usage: taskflow add \"My task\""))?;
            Command::Add {
                description: args.opt_value_from_str(["-d", "--description"])?,
                priority: args
                    .opt_value_from_str(["-p", "--priority"])?
                    .unwrap_or_else(|| "medium".to_string()),
                project: args.opt_value_from_str("--project")?,
                assignee: args.opt_value_from_str(["-a", "--assignee"])?,
                tags: args.opt_value_from_str(["-t", "--tags"])?,
                due: args.opt_value_from_str("--due")?,
                title,
            }
        }

        "list" | "ls" => Command::List {
            status: args.opt_value_from_str(["-s", "--status"])?,
            priority: args.opt_value_from_str(["-p", "--priority"])?,
            project: args.opt_value_from_str("--project")?,
            assignee: args.opt_value_from_str("--assignee")?,
            tag: args.opt_value_from_str("--tag")?,
            search: args.opt_value_from_str(["-q", "--search"])?,
            overdue: args.contains("--overdue"),
            format: args
                .opt_value_from_str("--format")?
                .unwrap_or_else(|| "table".to_string()),
            limit: args.opt_value_from_str(["-l", "--limit"])?,
        },

        "show" => {
            let id: String = args.free_from_str().map_err(|_| anyhow!("show requires a task ID"))?;
            Command::Show { id }
        }

        "done" => {
            let id: String = args.free_from_str().map_err(|_| anyhow!("done requires a task ID"))?;
            Command::Done { id }
        }

        "update" => {
            let id: String = args.free_from_str().map_err(|_| anyhow!("update requires a task ID"))?;
            Command::Update {
                id,
                title: args.opt_value_from_str("--title")?,
                status: args.opt_value_from_str(["-s", "--status"])?,
                priority: args.opt_value_from_str(["-p", "--priority"])?,
                assignee: args.opt_value_from_str(["-a", "--assignee"])?,
                tags: args.opt_value_from_str("--tags")?,
                due: args.opt_value_from_str("--due")?,
            }
        }

        "delete" | "rm" => {
            let id: String = args.free_from_str().map_err(|_| anyhow!("delete requires a task ID"))?;
            Command::Delete { id, yes: args.contains(["-y", "--yes"]) }
        }

        "project" => {
            let sub = args.subcommand()?.unwrap_or_default();
            match sub.as_str() {
                "add" => {
                    let name: String = args.free_from_str().map_err(|_| anyhow!("project add requires a name"))?;
                    Command::ProjectAdd {
                        name,
                        description: args.opt_value_from_str(["-d", "--description"])?,
                        color: args.opt_value_from_str("--color")?,
                    }
                }
                "list" | "ls" => Command::ProjectList,
                "delete" | "rm" => {
                    let id: String = args.free_from_str().map_err(|_| anyhow!("project delete requires an ID"))?;
                    Command::ProjectDelete { id, yes: args.contains(["-y", "--yes"]) }
                }
                other => return Err(anyhow!("Unknown project subcommand: '{}'. Use: add, list, delete", other)),
            }
        }

        "stats" => Command::Stats,

        "serve" => Command::Serve {
            host: args
                .opt_value_from_str("--host")?
                .unwrap_or_else(|| "127.0.0.1".to_string()),
            port: args
                .opt_value_from_str(["-p", "--port"])?
                .unwrap_or(8765u16),
        },

        "" | "help" => {
            print!("{}", HELP);
            std::process::exit(0);
        }

        other => {
            return Err(anyhow!(
                "Unknown command: '{}'\n\nRun `taskflow --help` for available commands.",
                other
            ))
        }
    };

    Ok((global, cmd))
}

pub async fn run(cmd: Command, db: &Database) -> Result<()> {
    match cmd {
        Command::Add { title, description, priority, project, assignee, tags, due } => {
            commands::task_add(db, title, description, priority, project, assignee, tags, due).await
        }
        Command::List { status, priority, project, assignee, tag, search, overdue, format, limit } => {
            commands::task_list(db, status, priority, project, assignee, tag, search, overdue, &format, limit).await
        }
        Command::Show { id } => commands::task_show(db, &id).await,
        Command::Done { id } => commands::task_done(db, &id).await,
        Command::Update { id, title, status, priority, assignee, tags, due } => {
            commands::task_update(db, &id, title, status, priority, assignee, tags, due).await
        }
        Command::Delete { id, yes } => commands::task_delete(db, &id, yes).await,
        Command::ProjectAdd { name, description, color } => {
            commands::project_add(db, name, description, color).await
        }
        Command::ProjectList => commands::project_list(db).await,
        Command::ProjectDelete { id, yes } => commands::project_delete(db, &id, yes).await,
        Command::Stats => commands::stats(db).await,
        Command::Serve { host, port } => crate::api::serve(&host, port, db.clone()).await,
    }
}
