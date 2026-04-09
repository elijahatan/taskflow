use anyhow::Result;
use log::{info, warn};
use std::io::Read;
use tiny_http::{Request, Response, Method};

use crate::db::Database;
use super::handlers;

pub struct ApiResponse {
    pub status: u16,
    pub body: String,
}

impl ApiResponse {
    pub fn ok(body: impl Into<String>) -> Self {
        Self { status: 200, body: body.into() }
    }
    pub fn created(body: impl Into<String>) -> Self {
        Self { status: 201, body: body.into() }
    }
    pub fn not_found(msg: &str) -> Self {
        Self {
            status: 404,
            body: format!(r#"{{"error":"{}"}}"#, msg),
        }
    }
    pub fn bad_request(msg: &str) -> Self {
        Self {
            status: 400,
            body: format!(r#"{{"error":"{}"}}"#, msg),
        }
    }
    pub fn internal_error(msg: &str) -> Self {
        Self {
            status: 500,
            body: format!(r#"{{"error":"{}"}}"#, msg),
        }
    }
}


pub fn handle_request(mut request: Request, db: &Database) -> Result<()> {
    let method = request.method().clone();
    let url = request.url().to_string();

    let path = url.split('?').next().unwrap_or("/");
    let query = url.split('?').nth(1).unwrap_or("");

    info!("{} {}", method, url);

    let mut body = String::new();
    request.as_reader().read_to_string(&mut body).unwrap_or(0);

    let response = route(&method, path, query, &body, db);

    let status_code = tiny_http::StatusCode(response.status);
    let http_response = Response::from_string(response.body)
        .with_status_code(status_code)
        .with_header(
            "Content-Type: application/json".parse::<tiny_http::Header>().unwrap(),
        )
        .with_header(
            "Access-Control-Allow-Origin: *".parse::<tiny_http::Header>().unwrap(),
        );

    request.respond(http_response).map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

fn route(method: &Method, path: &str, query: &str, body: &str, db: &Database) -> ApiResponse {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    match (method, segments.as_slice()) {
        (Method::Get, ["health"]) => {
            ApiResponse::ok(r#"{"status":"ok","service":"taskflow"}"#)
        }

        (Method::Get, ["api", "v1", "tasks"]) => {
            handlers::list_tasks(query, db)
        }
        (Method::Post, ["api", "v1", "tasks"]) => {
            handlers::create_task(body, db)
        }

        (Method::Get, ["api", "v1", "tasks", id]) => {
            handlers::get_task(id, db)
        }
        (Method::Put, ["api", "v1", "tasks", id]) => {
            handlers::update_task(id, body, db)
        }
        (Method::Delete, ["api", "v1", "tasks", id]) => {
            handlers::delete_task(id, db)
        }

        (Method::Post, ["api", "v1", "tasks", id, "done"]) => {
            handlers::complete_task(id, db)
        }

        (Method::Get, ["api", "v1", "projects"]) => {
            handlers::list_projects(db)
        }
        (Method::Post, ["api", "v1", "projects"]) => {
            handlers::create_project(body, db)
        }
        (Method::Delete, ["api", "v1", "projects", id]) => {
            handlers::delete_project(id, db)
        }

        (Method::Get, ["api", "v1", "stats"]) => {
            handlers::get_stats(db)
        }

        (Method::Options, _) => {
            ApiResponse::ok("{}")
        }

        _ => {
            warn!("404 {} {}", method, path);
            ApiResponse::not_found(&format!("No route for {} {}", method, path))
        }
    }
}
