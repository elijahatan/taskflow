use log::warn;

use crate::db::project_repo::ProjectRepository;
use crate::db::task_repo::TaskRepository;
use crate::db::Database;
use crate::models::project::CreateProjectRequest;
use crate::models::task::{
    CreateTaskRequest, TaskFilter, TaskPriority, TaskStatus, UpdateTaskRequest,
};

use super::router::ApiResponse;

#[derive(serde::Deserialize)]
struct DependencyRequest {
    depends_on_task_id: String,
}

pub fn list_tasks(query: &str, db: &Database) -> ApiResponse {
    let mut filter = TaskFilter::default();

    for pair in query.split('&').filter(|s| !s.is_empty()) {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        let val = kv.next().unwrap_or("").trim();

        match key {
            "status" => match TaskStatus::from_str(val) {
                Ok(s) => filter.status = Some(s),
                Err(e) => return ApiResponse::bad_request(&e.to_string()),
            },
            "priority" => match TaskPriority::from_str(val) {
                Ok(p) => filter.priority = Some(p),
                Err(e) => return ApiResponse::bad_request(&e.to_string()),
            },
            "project_id" => filter.project_id = Some(val.to_string()),
            "assignee" => filter.assignee = Some(val.to_string()),
            "tag" => filter.tag = Some(val.to_string()),
            "search" | "q" => filter.search = Some(url_decode(val)),
            "overdue" => filter.overdue_only = val == "true" || val == "1",
            "limit" => filter.limit = val.parse().ok(),
            "offset" => filter.offset = val.parse().ok(),
            _ => {}
        }
    }

    let repo = TaskRepository::new(db);
    match repo.list(&filter) {
        Ok(tasks) => match serde_json::to_string(&tasks) {
            Ok(json) => ApiResponse::ok(json),
            Err(e) => ApiResponse::internal_error(&e.to_string()),
        },
        Err(e) => {
            warn!("list_tasks error: {}", e);
            ApiResponse::internal_error(&e.to_string())
        }
    }
}

pub fn create_task(body: &str, db: &Database) -> ApiResponse {
    let req: CreateTaskRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return ApiResponse::bad_request(&format!("Invalid JSON: {}", e)),
    };

    let repo = TaskRepository::new(db);
    match repo.create(req) {
        Ok(task) => match serde_json::to_string(&task) {
            Ok(json) => ApiResponse::created(json),
            Err(e) => ApiResponse::internal_error(&e.to_string()),
        },
        Err(e) => {
            warn!("create_task error: {}", e);
            ApiResponse::bad_request(&e.to_string())
        }
    }
}

pub fn get_task(id: &str, db: &Database) -> ApiResponse {
    let repo = TaskRepository::new(db);
    match repo.get_by_id(id) {
        Ok(Some(task)) => match serde_json::to_string(&task) {
            Ok(json) => ApiResponse::ok(json),
            Err(e) => ApiResponse::internal_error(&e.to_string()),
        },
        Ok(None) => ApiResponse::not_found(&format!("Task '{}' not found", id)),
        Err(e) => ApiResponse::internal_error(&e.to_string()),
    }
}

pub fn get_task_activity(id: &str, db: &Database) -> ApiResponse {
    let repo = TaskRepository::new(db);
    if let Ok(None) = repo.get_by_id(id) {
        return ApiResponse::not_found(&format!("Task '{}' not found", id));
    }

    match repo.activity(id) {
        Ok(items) => match serde_json::to_string(&items) {
            Ok(json) => ApiResponse::ok(json),
            Err(e) => ApiResponse::internal_error(&e.to_string()),
        },
        Err(e) => ApiResponse::internal_error(&e.to_string()),
    }
}

pub fn update_task(id: &str, body: &str, db: &Database) -> ApiResponse {
    let req: UpdateTaskRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return ApiResponse::bad_request(&format!("Invalid JSON: {}", e)),
    };

    let repo = TaskRepository::new(db);
    match repo.update(id, req) {
        Ok(Some(task)) => match serde_json::to_string(&task) {
            Ok(json) => ApiResponse::ok(json),
            Err(e) => ApiResponse::internal_error(&e.to_string()),
        },
        Ok(None) => ApiResponse::not_found(&format!("Task '{}' not found", id)),
        Err(e) => {
            warn!("update_task error: {}", e);
            ApiResponse::bad_request(&e.to_string())
        }
    }
}

pub fn delete_task(id: &str, db: &Database) -> ApiResponse {
    let repo = TaskRepository::new(db);
    match repo.delete(id) {
        Ok(true) => ApiResponse::ok(format!(r#"{{"deleted":"{}"}}"#, id)),
        Ok(false) => ApiResponse::not_found(&format!("Task '{}' not found", id)),
        Err(e) => ApiResponse::internal_error(&e.to_string()),
    }
}

pub fn complete_task(id: &str, db: &Database) -> ApiResponse {
    let req = UpdateTaskRequest {
        status: Some(TaskStatus::Done),
        ..Default::default()
    };
    let repo = TaskRepository::new(db);
    match repo.update(id, req) {
        Ok(Some(task)) => match serde_json::to_string(&task) {
            Ok(json) => ApiResponse::ok(json),
            Err(e) => ApiResponse::internal_error(&e.to_string()),
        },
        Ok(None) => ApiResponse::not_found(&format!("Task '{}' not found", id)),
        Err(e) => ApiResponse::internal_error(&e.to_string()),
    }
}

pub fn add_dependency(id: &str, body: &str, db: &Database) -> ApiResponse {
    let req: DependencyRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return ApiResponse::bad_request(&format!("Invalid JSON: {}", e)),
    };

    let repo = TaskRepository::new(db);
    match repo.add_dependency(id, &req.depends_on_task_id) {
        Ok(task) => match serde_json::to_string(&task) {
            Ok(json) => ApiResponse::created(json),
            Err(e) => ApiResponse::internal_error(&e.to_string()),
        },
        Err(e) => ApiResponse::bad_request(&e.to_string()),
    }
}

pub fn remove_dependency(id: &str, depends_on_id: &str, db: &Database) -> ApiResponse {
    let repo = TaskRepository::new(db);
    match repo.remove_dependency(id, depends_on_id) {
        Ok(task) => match serde_json::to_string(&task) {
            Ok(json) => ApiResponse::ok(json),
            Err(e) => ApiResponse::internal_error(&e.to_string()),
        },
        Err(e) => ApiResponse::bad_request(&e.to_string()),
    }
}

pub fn list_projects(db: &Database) -> ApiResponse {
    let repo = ProjectRepository::new(db);
    match repo.list() {
        Ok(projects) => match serde_json::to_string(&projects) {
            Ok(json) => ApiResponse::ok(json),
            Err(e) => ApiResponse::internal_error(&e.to_string()),
        },
        Err(e) => ApiResponse::internal_error(&e.to_string()),
    }
}

pub fn create_project(body: &str, db: &Database) -> ApiResponse {
    let req: CreateProjectRequest = match serde_json::from_str(body) {
        Ok(r) => r,
        Err(e) => return ApiResponse::bad_request(&format!("Invalid JSON: {}", e)),
    };

    let repo = ProjectRepository::new(db);
    match repo.create(req) {
        Ok(project) => match serde_json::to_string(&project) {
            Ok(json) => ApiResponse::created(json),
            Err(e) => ApiResponse::internal_error(&e.to_string()),
        },
        Err(e) => ApiResponse::bad_request(&e.to_string()),
    }
}

pub fn delete_project(id: &str, db: &Database) -> ApiResponse {
    let repo = ProjectRepository::new(db);
    match repo.delete(id) {
        Ok(true) => ApiResponse::ok(format!(r#"{{"deleted":"{}"}}"#, id)),
        Ok(false) => ApiResponse::not_found(&format!("Project '{}' not found", id)),
        Err(e) => ApiResponse::internal_error(&e.to_string()),
    }
}

pub fn get_stats(db: &Database) -> ApiResponse {
    let repo = TaskRepository::new(db);
    match repo.statistics() {
        Ok(stats) => {
            let json = format!(
                r#"{{"total":{},"todo":{},"in_progress":{},"done":{},"overdue":{},"critical":{}}}"#,
                stats.total,
                stats.todo,
                stats.in_progress,
                stats.done,
                stats.overdue,
                stats.critical
            );
            ApiResponse::ok(json)
        }
        Err(e) => ApiResponse::internal_error(&e.to_string()),
    }
}

fn url_decode(s: &str) -> String {
    s.replace('+', " ")
        .split('%')
        .enumerate()
        .fold(String::new(), |mut acc, (i, chunk)| {
            if i == 0 {
                acc.push_str(chunk);
            } else if chunk.len() >= 2 {
                if let Ok(byte) = u8::from_str_radix(&chunk[..2], 16) {
                    acc.push(byte as char);
                    acc.push_str(&chunk[2..]);
                } else {
                    acc.push('%');
                    acc.push_str(chunk);
                }
            } else {
                acc.push('%');
                acc.push_str(chunk);
            }
            acc
        })
}
