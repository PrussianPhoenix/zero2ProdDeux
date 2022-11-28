use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use tracing::Instrument;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

//implement 'subscribe' handler
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>, //renamed
) -> HttpResponse {
    //Persist subscriber details
    // 'Result' of the query has two variants: 'Ok' and 'Err'.
    // The first for successes, the second for failures.
    // We use a 'match' statement to choose what to do based
    // on the outcome.
    // We will talk more about 'Result' going forward!
    let request_id = Uuid::new_v4();
    // Spans, like logs, have an associated level
    // 'info_span' creates a span at the info-level
    let request_span = tracing::info_span!(
      "Adding a new subscriber.",
        %request_id,
        subscriber_email = %form.email,
        subscriber_name = %form.name
    );

    //Using 'enter' in an async function is a recipe for disaster!

    let _request_span_guard = request_span.enter();
    // we'll drop at the end of the subscribe method via '.exit'

    //we do not call '.enter on query_span
    // .instrument takes care of it at the right moments
    // in the query future lifetime

    let query_span = tracing::info_span!(
        "Saving new subscriber details in the database"
    );

    match sqlx::query!(
        r#"
        INSERT INTO subscriptions (id,email,name, subscribed_at)
        VALUES ($1,$2,$3,$4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now()
        )
    // We use 'get_ref' to get an immutable reference to the 'PgPool' (formerly PgConnection)
    // wrapped by 'web::Data'.
    .execute(pool.get_ref())
    // attach the instrumentation and await it
    .instrument(query_span)
    .await
    {
        // handle 'ok' and 'err' paths
        Ok(_) => {
            HttpResponse::Ok().finish()
        },
        Err(e) => {
            // error log falls outside of 'query_span' currently
            tracing::error!("request_id {} - Failed to Execute query: {:?}",
                request_id,
                e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
