use actix_web::HttpResponse;
use actix_web::web;
use actix_web::web::Data;
use sqlx::PgPool;
use actix_web::http::{header, StatusCode};
use crate::email_client::EmailClient;
// use anyhow's extension trait into scope!
use anyhow::{anyhow, Context};
use crate::domain::SubscriberEmail;
use secrecy::Secret;
use actix_web_flash_messages::FlashMessage;
use crate::utils::{e500,see_other};

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(
name = "Publish a newsletter issue",
skip(form, pool,email_client)
)]
pub async fn publish_newsletter(
                                form: web::Form<FormData>,
                                pool: Data<PgPool>,
                                email_client: Data<EmailClient>,)
                                -> Result<HttpResponse, actix_web::Error> {
    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(&subscriber.email,
                                &form.title,
                                &form.html_content,
                                &form.text_content,
                    ).await
                    // lazy context method. Takes closure as an argument (so only when error)
                    .with_context(| | {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    }).map_err(e500)?;
            }

            Err(error) => {
                tracing::warn!(
                  // we record the error chain as a structured field
                    //on the log record.
                    error.cause_chain = ?error,
                    // using '\' to split a long string literal over
                    // two lines, without creating a '\n' character.
                    "Skipping a confirmed subscriber. \
                    Their stored contact details are invalid",
                );
            }
        }
    }
    /*
    old implementation where skip email logic was in get_confirmed_subscribers
     for subscriber in subscribers {
        email_client
            .send_email(subscriber.email,
            &body.title,
            &body.content.html,
            &body.content.text,
            ).await
            // lazy context method. Takes closure as an argument (so only when error)
            .with_context(|| {
                format!("Failed to send newsletter issue to {}", subscriber.email)
            })?;
    }
     */
    FlashMessage::info("The newsletter issue has been published!").send();
    Ok(see_other("/admin/newsletters"))
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(pool))]
async fn get_confirmed_subscribers(
    pool: &PgPool,
    // we are returning a 'vec' of 'results' in the happy case.
    // this allows the caller to bubble up errors due to network issues or other
    // failures using the '?' operator, while the compiler
    // forces them to handle the subtler mapping error.
    // see http://sled.rs/errors.html for a deep-dive on this technique.
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>,anyhow::Error> {
    // We only need 'row' to map the data coming out of this query.
    // Nesting its definition inside the function itself is a simple way
    // to clearly communicate this coupling (and to ensure it doesn't
    // get used elsewhere by mistake).

    /*
    struct Row{
        email:String,
    }

    let rows = sqlx::query_as!(Row,

    //now map into the domain type
    let confirmed_subscribers = rows.into_iter()
        .map(|r| match SubscriberEmail::parse(r.email){
    */
    let confirmed_subscribers = sqlx::query!(
        r#"
        SELECT email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
    )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|r| match SubscriberEmail::parse(r.email){
            Ok(email) => Ok(ConfirmedSubscriber { email }),
            Err(error) => Err(anyhow::anyhow!(error)),
            /*
            previous implementation where email skip and abort business logic was in this function.
            since moved to publish_newsletter()
            original return type
                'Result<Vec<ConfirmedSubscriber>,anyhow::Error>'

            let confirmed_subscribers = rows.into_iter().filter_map(|r| match SubscriberEmail::parse(r.email){
            Ok(email) => Some(ConfirmedSubscriber { email }),
            Err(error) => {
                tracing::warn!(
                    "A confirmed subscriber is using an invalid email address.\n{}.",
                    error
                );
                None
            }
             */
        })
        .collect();
    Ok(confirmed_subscribers)
}