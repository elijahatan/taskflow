use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use colored::*;

use crate::cli::display;
use crate::db::task_repo::TaskRepository;
use crate::db::project_repo::ProjectRepository;
use crate::db::Database;
use crate::models::project::CreateProjectRequest;
use crate::models::task::{CreateTaskRequest, TaskFilter, TaskPriority, TaskStatus, UpdateTaskRequest};

fn parse_tags(tags: Option<String>) -> Vec<String> {
    tags.map(|t| {
        t.split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

fn parse_due_date(s: &str) -> Result<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("Invalid date '{}'. Use YYYY-MM-DD format.", s))?;
    let dt = date
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| anyhow::anyhow!("Invalid date components"))?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc))
}

fn resolve_task_id(db: &Database, partial: &str) -> Result<String> {
    if partial.len() == 36 {
        return Ok(partial.to_string());
    }
    let repo = TaskRepository::new(db);
    let all = repo.list(&TaskFilter::default())?;
    let matches: Vec<_> = all.iter().filter(|t| t.id.starts_with(partial)).collect();
    match matches.len() {
        0 => Err(anyhow::anyhow!("No task found matching ID prefix '{}'", partial)),
        1 => Ok(matches[0].id.clone()),
        _ => Err(anyhow::anyhow!(
            "Ambiguous ID prefix '{}' matches {} tasks. Use more characters.",
            partial,
            matches.len()
        )),
    }
}

pub async fn task_add(
    db: &Database,
    title: String,
    description: Option<String>,
    priority: String,
    project: Option<String>,
    assignee: Option<String>,
    tags: Option<String>,
    due: Option<String>,
) -> Result<()> {
    let priority = TaskPriority::from_str(&priority)?;
    let due_date = due.as_deref().map(parse_due_date).transpose()?;

    let req = CreateTaskRequest {
        title,
        description,
        priority: Some(priority),
        project_id: project,
        assignee,
        tags: Some(parse_tags(tags)),
        due_date,
    };

    let repo = TaskRepository::new(db);
    let task = repo.create(req)?;

    display::success(&format!(
        "Created task {} — {}",
        &task.id[..8].yellow(),
        task.title.bold()
    ));
    Ok(())
}

pub async fn task_list(
    db: &Database,
    status: Option<String>,
    priority: Option<String>,
    project: Option<String>,
    assignee: Option<String>,
    tag: Option<String>,
    search: Option<String>,
    overdue: bool,
    format: &str,
    limit: Option<u32>,
) -> Result<()> {
    let filter = TaskFilter {
        status: status.as_deref().map(TaskStatus::from_str).transpose()?,
        priority: priority.as_deref().map(TaskPriority::from_str).transpose()?,
        project_id: project,
        assignee,
        tag,
        search,
        overdue_only: overdue,
        limit,
        offset: None,
    };

    let repo = TaskRepository::new(db);
    let tasks = repo.list(&filter)?;

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&tasks)?),
        _ => display::render_task_table(&tasks),
    }
    Ok(())
}

pub async fn task_show(db: &Database, id: &str) -> Result<()> {
    let full_id = resolve_task_id(db, id)?;
    let repo = TaskRepository::new(db);
    match repo.get_by_id(&full_id)? {
        Some(task) => display::render_task_detail(&task),
        None => display::error(&format!("Task '{}' not found.", id)),
    }
    Ok(())
}

pub async fn task_done(db: &Database, id: &str) -> Result<()> {
    let full_id = resolve_task_id(db, id)?;
    let repo = TaskRepository::new(db);
    let req = UpdateTaskRequest { status: Some(TaskStatus::Done), ..Default::default() };
    match repo.update(&full_id, req)? {
        Some(task) => display::success(&format!(
            "Marked {} — {} as {}",
            &task.id[..8].yellow(),
            task.title.bold(),
            "done".green().bold()
        )),
        None => display::error(&format!("Task '{}' not found.", id)),
    }
    Ok(())
}

pub async fn task_update(
    db: &Database,
    id: &str,
    title: Option<String>,
    status: Option<String>,
    priority: Option<String>,
    assignee: Option<String>,
    tags: Option<String>,
    due: Option<String>,
) -> Result<()> {
    let full_id = resolve_task_id(db, id)?;
    let req = UpdateTaskRequest {
        title,
        status: status.as_deref().map(TaskStatus::from_str).transpose()?,
        priority: priority.as_deref().map(TaskPriority::from_str).transpose()?,
        assignee,
        tags: tags.map(|t| parse_tags(Some(t))),
        due_date: due.as_deref().map(parse_due_date).transpose()?,
        ..Default::default()
    };
    let repo = TaskRepository::new(db);
    match repo.update(&full_id, req)? {
        Some(task) => display::success(&format!(
            "Updated task {} — {}",
            &task.id[..8].yellow(),
            task.title.bold()
        )),
        None => display::error(&format!("Task '{}' not found.", id)),
    }
    Ok(())
}

pub async fn task_delete(db: &Database, id: &str, yes: bool) -> Result<()> {
    let full_id = resolve_task_id(db, id)?;
    let repo = TaskRepository::new(db);
    if let Some(task) = repo.get_by_id(&full_id)? {
        if !yes {
            println!(
                "  Delete task {} — {}? [y/N] ",
                &task.id[..8].yellow(),
                task.title.bold()
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("{}", "Cancelled.".dimmed());
                return Ok(());
            }
        }
        if repo.delete(&full_id)? {
            display::success(&format!("Deleted task {}", &full_id[..8].yellow()));
        }
    } else {
        display::error(&format!("Task '{}' not found.", id));
    }
    Ok(())
}

pub async fn stats(db: &Database) -> Result<()> {
    let repo = TaskRepository::new(db);
    let stats = repo.statistics()?;
    display::render_stats(&stats);
    Ok(())
}

pub async fn project_add(
    db: &Database,
    name: String,
    description: Option<String>,
    color: Option<String>,
) -> Result<()> {
    let repo = ProjectRepository::new(db);
    let project = repo.create(CreateProjectRequest { name, description, color })?;
    display::success(&format!(
        "Created project {} — {}",
        &project.id[..8].yellow(),
        project.name.bold()
    ));
    Ok(())
}

pub async fn project_list(db: &Database) -> Result<()> {
    let repo = ProjectRepository::new(db);
    let projects = repo.list()?;
    if projects.is_empty() {
        println!("{}", "No projects yet. Use `taskflow project add <name>`.".dimmed());
    } else {
        println!();
        println!(
            "  {:<38} {:<24} {}",
            "ID".bold(),
            "Name".bold(),
            "Description".bold()
        );
        println!("  {}", "─".repeat(72).dimmed());
        for p in &projects {
            let desc = p.description.as_deref().unwrap_or("—");
            println!("  {:<38} {:<24} {}", p.id, p.name, desc.dimmed());
        }
        println!("\n  {} project(s)", projects.len().to_string().bold());
    }
    Ok(())
}

pub async fn project_delete(db: &Database, id: &str, yes: bool) -> Result<()> {
    let repo = ProjectRepository::new(db);
    if let Some(p) = repo.get_by_id(id)? {
        if !yes {
            println!(
                "  Delete project {} — {}? [y/N] ",
                &p.id[..8].yellow(),
                p.name.bold()
            );
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("{}", "Cancelled.".dimmed());
                return Ok(());
            }
        }
        if repo.delete(id)? {
            display::success(&format!("Deleted project {}", &id[..8].yellow()));
        }
    } else {
        display::error(&format!("Project '{}' not found.", id));
    }
    Ok(())
}
