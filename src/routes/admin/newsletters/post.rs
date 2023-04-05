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
use crate::authentication::UserId;
use crate::utils::{e400, e500,see_other};
use crate::idempotency::IdempotencyKey;
use crate::idempotency::get_saved_response;
use crate::idempotency::save_response;
use crate::idempotency::{try_processing, NextAction};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    //new field
    idempotency_key: String,
}

#[tracing::instrument(
name = "Publish a newsletter issue",
skip(form, pool,email_client)
)]
pub async fn publish_newsletter(
                                form: web::Form<FormData>,
                                pool: Data<PgPool>,
                                email_client: Data<EmailClient>,
                                // inject the user id extracted from the user session
                                user_id: web::ReqData<UserId>,)
                                -> Result<HttpResponse, actix_web::Error> {
    let user_id = user_id.into_inner();
    // destructure the form to avoid upsetting the borrow-checker
    let FormData {title, text_content, html_content, idempotency_key} = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;
    let transaction = match try_processing(&pool, &idempotency_key, *user_id)
        .await
        .map_err(e500)?
    {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            return Ok(saved_response)
        }
    };
/*
    //return early if we have a saved response in the database
    if let Some(saved_response) = get_saved_response(
        &pool,
        &idempotency_key,
        *user_id
    )
        .await
        .map_err(e500)?
    {
        FlashMessage::info("The newsletter issue has been published!").send();
        return Ok(saved_response);
    }
*/
    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email(&subscriber.email,
                                &title,
                                &html_content,
                                &text_content,
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
    success_message().send();
    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, *user_id, response)
        .await
        .map_err(e500)?;
    Ok(response)
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

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been published!")
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    sqlx::query!(
        r#"
            INSERT INTO newsletter_issues (
                newsletter_issue_id,
                title,
                text_content,
                html_content,
                published_at
            )
            VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    )
        .execute(transaction)
        .await?;
    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id,
            subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id,
    )
        .execute(transaction)
        .await?;
    Ok(())
}