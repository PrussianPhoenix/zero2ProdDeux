use actix_web::{App, HttpRequest, Responder};
use std::net::TcpListener;
use secrecy::ExposeSecret;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use zero2Prod::configuration::get_configuration;
use zero2Prod::telemetry::{get_subscriber,init_subscriber};
use zero2Prod::email_client::EmailClient;

async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", &name)
}

use zero2Prod::startup::run;
use zero2Prod::startup::{Application};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let configuration = get_configuration().expect("Failed to read configuration");
    /*
    // Renamed!
    let connection_pool = PgPoolOptions::new().acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.database.connection_string());
    /*
    let connection = PgConnection::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to Postgres.");
     */
    let sender_email = configuration.email_client.sender()
        .expect("Invalid sender email address.");

    let timeout = configuration.email_client.timeout();

    let email_client = EmailClient::new(configuration.email_client.base_url,
    sender_email,
        configuration.email_client.authorization_token,
        // pass new argument from configuration
        timeout,
    );

    let address = format!("{}:{}", configuration.application.host ,configuration.application.port);
    // Bubble up the io::Error if we failed to bind the address
    // Otherwise call .await on our Server
    let listener = TcpListener::bind(address)?;
    run(listener, connection_pool, email_client)?.await?;

    let server = build(configuration).await?;
    server.await?;
    */
    let application = Application::build(configuration)
        .await?;
    application.run_until_stopped().await?;
    Ok(())
}
