use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use std::net::TcpListener;
use sqlx::{PgPool};
use tracing_actix_web::TracingLogger;

use crate::routes::{health_check, send_confirmation_email, subscribe, publish_newsletter};
use actix_web::{ HttpRequest, Responder};
use actix_web::web::Data;
use crate::email_client::EmailClient;

use crate::configuration::{get_configuration, Settings, DatabaseSettings};
use sqlx::postgres::PgPoolOptions;
use crate::routes::confirm;
use crate::routes::home;
use crate::routes::login_form;
use crate::routes::login;


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

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {

    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        /*let connection_pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect_lazy_with(configuration.database.with_db());
        */
        let connection_pool = get_connection_pool(&configuration.database);
        let sender_email = configuration.email_client.sender().expect("Invalid sender email address.");
        let timeout = configuration.email_client.timeout();
        let email_client = EmailClient::new(configuration.email_client.base_url,
                                            sender_email,
                                            configuration.email_client.authorization_token,
                                            timeout,
        );

        let address = format!("{}:{}", configuration.application.host ,configuration.application.port);
        // Bubble up the io::Error if we failed to bind the address
        // Otherwise call .await on our Server
        let listener = TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(listener, connection_pool, email_client,
        // new parameter
        configuration.application.base_url,)?;

        Ok(Self {port, server})
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    //expressive function that returns when the application is stopped
    pub async fn run_until_stopped(self) -> Result <(), std::io::Error> {
        self.server.await
    }
}

pub fn get_connection_pool(configuration: &DatabaseSettings) ->PgPool {
    PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.with_db())
}

// we need to define a wrapper type in order to retrieve the url
// in the 'subscribe' handler.
// Retrieval from the context, in actix web is type based: using
// a raw 'String' would expose us to conflicts
pub struct ApplicationBaseUrl(pub String);

// Notice the different signature!
// We return `Server` on the happy path and we dropped the `async` keyword
// We have no .await call, so it is not needed anymore.

pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
    // new parameter
    base_url: String,
) -> Result<Server, std::io::Error> {
    // Wrap the connection in a smart pointer
    // Wrap the pool using web::data, which boils down to an Arc smart pointer
    let db_pool = Data::new(db_pool);
    let email_client = Data::new(email_client);
    let base_url = Data::new(ApplicationBaseUrl(base_url));
    // Capture 'connection' from the surrounding environment
    let server = HttpServer::new(move || {
        App::new()
            // middleware is added by using .wrap() on an app
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            // A new entry in our routing table for POST /subscriptions requests
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(publish_newsletter))
            //.route("/{name}", web::get().to(greet))
            .route("/", web::get().to(home))
            .route("/login", web::post().to(login))
            // Get a pointer copy and attach it to the application state
            .app_data(db_pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
    })
    .listen(listener)?
    .run();
    // No .await here
    Ok(server)
}
