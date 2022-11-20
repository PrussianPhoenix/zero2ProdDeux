use actix_web::{HttpRequest, Responder};
use std::net::TcpListener;
use sqlx::{PgPool};
use zero2Prod::configuration::get_configuration;

async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", &name)
}

use zero2Prod::startup::run;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let configuration = get_configuration().expect("Failed to read configuration");
    // Renamed!
    let connection_pool = PgPool::connect(&configuration.database.connection_string()
    )   .await
        .expect("Failed to connect to Postgres.");
    /*
    let connection = PgConnection::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to Postgres.");
     */
    let address = format!("127.0.0.1:{}", configuration.application_port);
    // Bubble up the io::Error if we failed to bind the address
    // Otherwise call .await on our Server
    let listener = TcpListener::bind(address)?;
    run(listener, connection_pool)?.await
}
