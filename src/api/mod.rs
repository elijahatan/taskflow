mod handlers;
mod router;

use anyhow::Result;
use colored::*;
use log::info;

use crate::db::Database;

pub use router::handle_request;


pub async fn serve(host: &str, port: u16, db: Database) -> Result<()> {
    let addr = format!("{}:{}", host, port);

    println!();
    println!("{}", "  TaskFlow API Server  ".on_blue().white().bold());
    println!("  Listening on {}", format!("http://{}", addr).cyan().bold());
    println!("  Press Ctrl+C to stop");
    println!();
    println!("  Routes:");
    println!("    GET  /health");
    println!("    GET  /api/v1/tasks");
    println!("    POST /api/v1/tasks");
    println!("    GET  /api/v1/tasks/:id");
    println!("    PUT  /api/v1/tasks/:id");
    println!("    DELETE /api/v1/tasks/:id");
    println!("    GET  /api/v1/tasks/:id/done");
    println!("    GET  /api/v1/projects");
    println!("    POST /api/v1/projects");
    println!("    GET  /api/v1/stats");
    println!();

    let server = tiny_http::Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("Failed to bind {}: {}", addr, e))?;

    info!("API server started on {}", addr);

    tokio::task::spawn_blocking(move || {
        for request in server.incoming_requests() {
            let db_clone = db.clone();
            match router::handle_request(request, &db_clone) {
                Ok(_) => {}
                Err(e) => log::error!("Request handler error: {}", e),
            }
        }
    })
    .await
    .map_err(|e| anyhow::anyhow!("Server thread panicked: {}", e))?;

    Ok(())
}
