use std::net::TcpListener;
use actix_web::App;
use once_cell::sync::Lazy;
use sqlx::{Connection,Executor, PgConnection, PgPool};
use zero2Prod::configuration::{get_configuration, DatabaseSettings};
use sqlx::types::Uuid;
use zero2Prod::email_client::EmailClient;
use zero2Prod::telemetry::{get_subscriber, init_subscriber};
use zero2Prod::startup::{get_connection_pool, Application};

// Ensure that the 'tracing' stack is only initialised once using 'once_cell'
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    // We cannot assing the output of 'get_subscriber' to a variable based on the
    // valuye TEST_LOG' because the sink is part of the type returned by get subscriber
    // therefore they are not the same type
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name,
                                        default_filter_level,
                                        std::io::stdout
        );
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name,
                                        default_filter_level,
                                        std::io::sink);
        init_subscriber(subscriber);
    };
});

// Generalise spawn_App
pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}



// Launch our application in the background ~somehow~

// No .await call, therefore no need for `spawn_app` to be async now.
// We are also running tests, so it is not worth it to propagate errors:
// if we fail to perform the required setup we can just panic and crash
// all the things.

//public now!
pub async fn spawn_app() -> TestApp {
    // the first time 'initialize' is invoked the code in 'TRAICNG' is executed.
    // All other invocations will instead skip execution.
    Lazy::force(&TRACING);
    /*
    //zero2Prod::run().await
    let listener = TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind random port");
    // We retrieve the port assigned to us by the OS
    let port = listener.local_addr().unwrap().port();

    let address = format!("http://127.0.0.1:{}", port);
    */

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration.");
        c.database.database_name = Uuid::new_v4().to_string();
        c.application.port = 0;
        c
    };

    configure_database(&configuration.database).await;

    //let connection_pool = configure_database(&configuration.database).await;
    /*
    let connection_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to Postgres.");
    */
/*
    //Build new email client
    let sender_email = configuration.email_client.sender().expect("Invalid sender email address.");
    let timeout = configuration.email_client.timeout();
    let email_client = EmailClient::new(configuration.email_client.base_url, sender_email, configuration.email_client.authorization_token, timeout);

    let server = zero2Prod::startup::run(listener, connection_pool.clone(), email_client)
        .expect("Failed to bind address");

 */
    //let server = build(configuration).await.expect("Failed to build application.");

    let application = Application::build(configuration.clone()).await
        .expect("Failed to build application.");
    let address = format!("http://127.0.0.1:{}", application.port());
    // Launch the server as a background task
    // tokio::spawn returns a handle to the spawned future,
    // but we have no use for it here, hence the non-binding let
    let _ = tokio::spawn(application.run_until_stopped());
    TestApp {
        address,
        db_pool: get_connection_pool(&configuration.database),
    }
    // We return the application address to the caller!
    //format!("http://127.0.0.1:{}", port)
}

//not public anymore
async fn configure_database(config: &DatabaseSettings) -> PgPool {
    // Create Database
    let mut connection = PgConnection::connect_with(
        &config.without_db())
        .await
        .expect("Failed to connect to Postgres");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    // Migrate database
    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to Postgres.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");
    connection_pool
}
