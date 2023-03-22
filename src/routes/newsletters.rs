use std::borrow::Borrow;
use actix_web::HttpResponse;
use actix_web::web;
use actix_web::web::Data;
use sqlx::PgPool;
use actix_web::ResponseError;
use crate::routes::error_chain_fmt;
use actix_web::http::{header, StatusCode};
use crate::email_client::EmailClient;
// use anyhow's extension trait into scope!
use anyhow::{anyhow, Context};
use crate::domain::SubscriberEmail;
use secrecy::Secret;
use actix_web::HttpRequest;
use actix_web::http::header::{HeaderMap, HeaderValue};
use base64::Engine;
use secrecy::ExposeSecret;
//use sha3::Digest;
//use argon2::{Algorithm, Argon2, Version, Params};
//use argon2::PasswordHasher;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use crate::telemetry::spawn_blocking_with_tracing;
use crate::authentication::{validate_credentials, AuthError, Credentials};


#[derive(thiserror::Error)]
pub enum PublishError {
    // new error variant!
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for PublishError{
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {HttpResponse::new(StatusCode::INTERNAL_SERVER_ERROR)}
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#)
                    .unwrap();
                response
                    .headers_mut()
                // actix_web::http::header provides a collection of constants
                // for the names of several well-known/standard HTTP headers
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            }
        }
    }
    // status code is invoked by the default 'error_response'
    // implementation. we are providing a bespoke 'error_response' implementation
    // therefore there is no need to maintain a 'status_code' implementation anymore.

    /*
    shift from status code for each error to a header.
    fn status_code(&self) -> StatusCode {
        match self {
            PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            // Return a 401 for auth errors
            PublishError::AuthError(_) => StatusCode::UNAUTHORIZED,
        }
    }
    */
}

//dummy implementation
//prefix _ to avoid unused variable compiler complaint
pub async fn publish_newsletter(body: web::Json<BodyData>,
                                pool: Data<PgPool>,
                                email_client: Data<EmailClient>,
                                request: HttpRequest,)
                                -> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers())
        .map_err(PublishError::AuthError)?;
    // add logging to check who is authenticating
    tracing::Span::current().record(
        "username",
        &tracing::field::display(&credentials.username)
    );
    let user_id = validate_credentials(credentials, &pool).await
        // we match on 'authError' vairants, but we pass the *whole* error
        // into the constructors for 'PublishError' Variants. This ensures that
        // the context of the top-level wrapper is preserved when the error is
        // logged by our middleware
        .map_err(|e| match e {
            AuthError::InvalidCredentials(_) => PublishError::AuthError(e.into()),
            AuthError::UnexpectedError(_) => PublishError::UnexpectedError(e.into()),
        })?;

    tracing::Span::current().record("user_id", &tracing::field::display(&user_id));
    let subscribers = get_confirmed_subscribers(&pool).await?;
    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                    email_client
                    .send_email(subscriber.email.borrow(),
                                &body.title,
                                &body.content.html,
                                &body.content.text,
                    ).await
                    // lazy context method. Takes closure as an argument (so only when error)
                    .with_context(| | {
                                  format!("Failed to send newsletter issue to {}", subscriber.email)
                                  }) ?;
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
    Ok(HttpResponse::Ok().finish())
}

fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    // the header value, if present, must be a valid UTF8 string
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header was missing")?
        .to_str()
        .context("The 'Authorization' header was not a valid UTF8 string.")?;

    let base64encoded_credentials = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic'.")?;

    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(base64encoded_credentials)
        .context("Failed to base64-decode 'Basic' credentials.")?;

    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The decoded credential string is not valid UTF8.")?;

    // Split into two segments, using ':' as delimiter
    let mut credentials = decoded_credentials.splitn(2, ':');
    let username = credentials
        .next()
        .ok_or_else(|| {
            anyhow::anyhow!("A username must be provided in 'Basic' auth.")
        })?
        .to_string();
    let password = credentials
        .next()
        .ok_or_else(|| {
           anyhow::anyhow!("A password must be provided in 'Basic' auth.")
        })?
        .to_string();

    Ok(Credentials{
        username,
        password: Secret::new(password)
    })
}

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String,
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