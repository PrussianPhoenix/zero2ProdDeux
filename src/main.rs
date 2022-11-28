use actix_web::{HttpRequest, Responder};
use std::net::TcpListener;
use sqlx::{PgPool};
use tracing::Subscriber;
use zero2Prod::configuration::get_configuration;
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};
use tracing_log::LogTracer;

async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", &name)
}

use zero2Prod::startup::run;

// Compose multiple layers into a 'tracing' subscriber.
//
// # implementation notes
//
// we are using 'impl subscriber' as return type yo avoid having to
// spell out the actual type of the returned subscriber, which is
// indeed quite complex.
// we need to explicitly call out that the returned subscriber is
// 'send' and 'sync' to make it possible to pass it to 'init_subscriber'
// later on.
/*
pub fn get_subscriber(name: String, env_filter: String) -> impl Subscriber + Send + Sync {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(env_filter));

    let formatting_layer = BunyanFormattingLayer::new(
        name,
        // output the formatted spans to stdout.
        std::io::stdout
    );
    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer);
}

//Regsiter a subscriber as global default to process span data.
//
// it should only be called once!
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber")
}
*/

#[tokio::main]
async fn main() -> std::io::Result<()> {
    //let subscriber = get_subscriber("zero2prod".into(), "info".into());
    //init_subscriber(subscriber);
    // redirect all 'log's events to our subcriber
    LogTracer::init().expect("Failed to set logger");
    //env_logger gone
    //we are printing all spans at info-level or above
    //if the rust_log env variable has not been set.
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let formatting_layer = BunyanFormattingLayer::new(
        "zero2prod".into(),
        // output the formatted spans to stdout.
        std::io::stdout
    );
    // The 'with' method is provided by 'SubscriberExt', an extension
    // trait for 'subscriber' exposed by 'tracing_subscriber'

    let subscriber = Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer);
    // 'set_global_default' can be used by applications to specify
    // what subscriber should be used to process spans.
    set_global_default(subscriber).expect("Failed to set subscriber");

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
