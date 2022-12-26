use actix_web::{HttpRequest, Responder};
use std::net::TcpListener;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use zero2Prod::configuration::get_configuration;
use zero2Prod::telemetry::{get_subscriber,init_subscriber};

async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", &name)
}

use zero2Prod::startup::run;


#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration");
    // Renamed!
    let connection_pool = PgPoolOptions::new().acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.database.connection_string());
    /*
    let connection = PgConnection::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to Postgres.");
     */
    let address = format!("{}:{}", configuration.application.host ,configuration.application.port);
    // Bubble up the io::Error if we failed to bind the address
    // Otherwise call .await on our Server
    let listener = TcpListener::bind(address)?;
    run(listener, connection_pool)?.await
}
