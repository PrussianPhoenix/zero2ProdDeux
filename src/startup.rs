use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use std::net::TcpListener;
use sqlx::{PgPool};
use tracing_actix_web::TracingLogger;

use crate::routes::{health_check, subscribe};
use actix_web::{ HttpRequest, Responder};


// We need to mark `run` as public.
// It is no longer a binary entrypoint, therefore we can mark it as async
// without having to use any proc-macro incantation.
/*
pub async fn run() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .route("/health_check", web::get().to(health_check))
    })
        .bind("127.0.0.1:8000")?
        .run()
        .await
}
*/

async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", &name)
}

// Notice the different signature!
// We return `Server` on the happy path and we dropped the `async` keyword
// We have no .await call, so it is not needed anymore.

pub fn run(
    listener: TcpListener,
    db_pool: PgPool
) -> Result<Server, std::io::Error> {
    // Wrap the connection in a smart pointer
    // Wrap the pool using web::data, which boils down to an Arc smart pointer
    let db_pool = web::Data::new(db_pool);
    // Capture 'connection' from the surrounding environment
    let server = HttpServer::new(move || {
        App::new()
            // middleware is added by using .wrap() on an app
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            // A new entry in our routing table for POST /subscriptions requests
            .route("/subscriptions", web::post().to(subscribe))
            .route("/{name}", web::get().to(greet))
            // Get a pointer copy and attach it to the application state
            .app_data(db_pool.clone())
    })
    .listen(listener)?
    .run();
    // No .await here
    Ok(server)
}
