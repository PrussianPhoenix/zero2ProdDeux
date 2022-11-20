use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;

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
    // We use 'get_ref' to get an immutable refernce to the 'PgPool' (formerly PgConnection)
    // wrapped by 'web::Data'.
    .execute(pool.get_ref())
    .await
    {
        // handle 'ok' and 'err' paths
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            println!("Failed to Execute query: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}
