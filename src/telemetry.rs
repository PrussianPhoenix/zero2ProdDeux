use tracing::Subscriber;
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};
use tracing_log::LogTracer;
use tracing_subscriber::fmt::MakeWriter;
use tokio::task::JoinHandle;

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

pub fn get_subscriber<Sink>(name: String, env_filter: String, sink:Sink,) -> impl Subscriber + Send + Sync
    where
        // ^ this "weird" syntax is a higher-ranked trait bound (hrtb)
        // it basically means that Sink implements the 'MakeWriter'
        // trait for all choices of the lifetime parameter `'a`
        // doc.rust-lang.org/nomicon/hrtb.html
        // for more
        Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
    {

    //we are printing all spans at info-level or above
    //if the rust_log env variable has not been set.
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(env_filter));

    let formatting_layer = BunyanFormattingLayer::new(
        name,
        // output the formatted spans to stdout.
        sink
    );
    // The 'with' method is provided by 'SubscriberExt', an extension
    // trait for 'subscriber' exposed by 'tracing_subscriber'
    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}

// Register a subscriber as global default to process span data.
//
// it should only be called once!
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    // redirect all 'log's events to our subscriber
    LogTracer::init().expect("Failed to set logger");

// 'set_global_default' can be used by applications to specify
// what subscriber should be used to process spans.
    set_global_default(subscriber).expect("Failed to set subscriber")
}

// just copied trait bounds and signature from 'spawn_blocking'
pub fn spawn_blocking_with_tracing<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let current_span = tracing::Span::current();
    tokio::task::spawn_blocking(move || current_span.in_scope(f))
}