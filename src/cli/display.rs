use colored::*;
use prettytable::{format, row, Cell, Row, Table};

use crate::models::task::{Task, TaskPriority, TaskRecurrence, TaskStatus};

pub fn priority_colored(p: &TaskPriority) -> ColoredString {
    match p {
        TaskPriority::Critical => "CRITICAL".red().bold(),
        TaskPriority::High => "HIGH".yellow().bold(),
        TaskPriority::Medium => "medium".normal(),
        TaskPriority::Low => "low".dimmed(),
    }
}

pub fn status_colored(s: &TaskStatus) -> ColoredString {
    match s {
        TaskStatus::Done => "done".green(),
        TaskStatus::InProgress => "in_progress".cyan(),
        TaskStatus::Blocked => "blocked".red(),
        TaskStatus::Cancelled => "cancelled".dimmed().strikethrough(),
        TaskStatus::Todo => "todo".normal(),
    }
}

pub fn recurrence_colored(r: &TaskRecurrence) -> ColoredString {
    match r {
        TaskRecurrence::Daily => "daily".blue(),
        TaskRecurrence::Weekly => "weekly".blue(),
        TaskRecurrence::Monthly => "monthly".blue(),
        TaskRecurrence::Yearly => "yearly".blue(),
    }
}

pub fn render_task_table(tasks: &[Task]) {
    if tasks.is_empty() {
        println!("{}", "No tasks found.".dimmed());
        return;
    }

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);

    table.set_titles(row![
        b -> "ID",
        b -> "Title",
        b -> "Status",
        b -> "Priority",
        b -> "Repeat",
        b -> "Due",
        b -> "Tags",
    ]);

    for task in tasks {
        let id_short = task.id[..8].to_string();
        let title = if task.title.len() > 40 {
            format!("{}…", &task.title[..39])
        } else {
            task.title.clone()
        };

        let due_str = match &task.due_date {
            Some(d) => {
                let s = d.format("%Y-%m-%d").to_string();
                if task.is_overdue() {
                    s.red().to_string()
                } else {
                    s
                }
            }
            None => "-".dimmed().to_string(),
        };
        let repeat_str = task
            .recurrence
            .as_ref()
            .map(|r| recurrence_colored(r).to_string())
            .unwrap_or_else(|| "-".dimmed().to_string());

        let tags_str = if task.tags.is_empty() {
            String::new()
        } else {
            task.tags.join(", ")
        };

        let id_display = if task.is_overdue() {
            format!("{} !", id_short).red().to_string()
        } else {
            id_short
        };

        table.add_row(Row::new(vec![
            Cell::new(&id_display),
            Cell::new(&title),
            Cell::new(&status_colored(&task.status).to_string()),
            Cell::new(&priority_colored(&task.priority).to_string()),
            Cell::new(&repeat_str),
            Cell::new(&due_str),
            Cell::new(&tags_str),
        ]));
    }

    table.printstd();
    println!("\n  {} task(s) shown", tasks.len().to_string().bold());
}

pub fn render_task_detail(task: &Task) {
    println!();
    println!("{}", "━".repeat(60).dimmed());
    println!("  {} {}", "Task:".bold(), task.title.bold().white());
    println!("{}", "━".repeat(60).dimmed());
    println!("  {:12} {}", "ID:".dimmed(), task.id);
    println!(
        "  {:12} {}",
        "Status:".dimmed(),
        status_colored(&task.status)
    );
    println!(
        "  {:12} {}",
        "Priority:".dimmed(),
        priority_colored(&task.priority)
    );
    if let Some(recurrence) = &task.recurrence {
        println!(
            "  {:12} {}",
            "Repeat:".dimmed(),
            recurrence_colored(recurrence)
        );
    }

    if let Some(desc) = &task.description {
        println!("  {:12} {}", "Description:".dimmed(), desc);
    }
    if let Some(project) = &task.project_id {
        println!("  {:12} {}", "Project:".dimmed(), project);
    }
    if let Some(assignee) = &task.assignee {
        println!("  {:12} {}", "Assignee:".dimmed(), assignee);
    }
    if !task.tags.is_empty() {
        println!(
            "  {:12} {}",
            "Tags:".dimmed(),
            task.tags.join(", ").cyan().to_string()
        );
    }
    if !task.blocked_by.is_empty() {
        let deps = task
            .blocked_by
            .iter()
            .map(|id| id.chars().take(8).collect::<String>())
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {:12} {}", "Blocked by:".dimmed(), deps.red());
    }
    if !task.blocks.is_empty() {
        let dependents = task
            .blocks
            .iter()
            .map(|id| id.chars().take(8).collect::<String>())
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {:12} {}", "Blocks:".dimmed(), dependents.yellow());
    }
    if let Some(due) = &task.due_date {
        let due_str = due.format("%Y-%m-%d %H:%M UTC").to_string();
        if task.is_overdue() {
            println!(
                "  {:12} {} {}",
                "Due:".dimmed(),
                due_str.red(),
                "⚠ OVERDUE".red().bold()
            );
        } else if let Some(days) = task.days_until_due() {
            println!("  {:12} {} (in {} days)", "Due:".dimmed(), due_str, days);
        } else {
            println!("  {:12} {}", "Due:".dimmed(), due_str);
        }
    }
    if let Some(completed) = &task.completed_at {
        println!(
            "  {:12} {}",
            "Completed:".dimmed(),
            completed.format("%Y-%m-%d %H:%M UTC")
        );
    }
    println!(
        "  {:12} {}",
        "Created:".dimmed(),
        task.created_at.format("%Y-%m-%d %H:%M UTC")
    );
    println!(
        "  {:12} {}",
        "Updated:".dimmed(),
        task.updated_at.format("%Y-%m-%d %H:%M UTC")
    );
    if !task.activities.is_empty() {
        println!("{}", "━".repeat(60).dimmed());
        println!("  {}", "Activity".bold());
        for activity in &task.activities {
            let ts = activity.created_at.format("%Y-%m-%d %H:%M").to_string();
            match &activity.details {
                Some(details) => {
                    println!("  {}  {}  {}", ts.dimmed(), activity.action.bold(), details)
                }
                None => println!("  {}  {}", ts.dimmed(), activity.action.bold()),
            }
        }
    }
    println!("{}", "━".repeat(60).dimmed());
    println!();
}

pub fn success(msg: &str) {
    println!("{} {}", "✓".green().bold(), msg);
}

pub fn error(msg: &str) {
    eprintln!("{} {}", "✗".red().bold(), msg);
}

#[allow(dead_code)]
pub fn warn(msg: &str) {
    println!("{} {}", "⚠".yellow().bold(), msg);
}

pub fn render_stats(stats: &crate::db::task_repo::TaskStatistics) {
    println!();
    println!("{}", "  TaskFlow Dashboard  ".on_blue().white().bold());
    println!();
    println!("  {:>6}  total tasks", stats.total.to_string().bold());
    println!("  {:>6}  to do", stats.todo.to_string().bold());
    println!(
        "  {:>6}  in progress",
        stats.in_progress.to_string().cyan().bold()
    );
    println!("  {:>6}  done", stats.done.to_string().green().bold());
    if stats.overdue > 0 {
        println!(
            "  {:>6}  {}",
            stats.overdue.to_string().red().bold(),
            "OVERDUE".red().bold()
        );
    }
    if stats.critical > 0 {
        println!(
            "  {:>6}  {}",
            stats.critical.to_string().red().bold(),
            "critical (open)".red()
        );
    }
    if stats.total > 0 {
        let pct = stats.done * 100 / stats.total;
        let bar_len = 30usize;
        let filled = (pct as usize * bar_len / 100).min(bar_len);
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_len - filled);
        println!();
        println!("  Progress  {}  {}%", bar.green(), pct);
    }
    println!();
}
