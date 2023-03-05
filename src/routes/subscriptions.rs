use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::Utc;
use tracing::Instrument;
// an extension trait to provide the 'graphemes' method
// on 'String' and '&str'
use unicode_segmentation::UnicodeSegmentation;
use crate::domain::{NewSubscriber, SubscriberName, SubscriberEmail};
use crate::email_client::EmailClient;

//access application_base_url in request handler
use crate::startup::ApplicationBaseUrl;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        // 'web::Form' is a wrapper around 'FormData'
        // 'form.0' gives us access to the underlying 'FormData'
        let email = SubscriberEmail::parse(value.email)?;
        Ok(Self{email, name})
    }
}
/*
pub fn parse_subscriber(form: FormData) -> Result<NewSubscriber, String> {
    let name = SubscriberName::parse(form.name)?;
    // 'web::Form' is a wrapper around 'FormData'
    // 'form.0' gives us access to the underlying 'FormData'
    let email = SubscriberEmail::parse(form.email)?;
    Ok(NewSubscriber{email, name})
}
*/
#[allow(clippy::async_yields_async)]
#[tracing::instrument (
    name = "Adding a new subscriber",
    skip(form, pool, email_client, base_url),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]
//Orchestrate the work to be done (database insertion) via routines/methods
// then take care of the web/http response according to its rules
//implement 'subscribe' handler
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>, //renamed
    //get email client from app context
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> HttpResponse {
    let new_subscriber = match form.0.try_into(){
        Ok(form) => form,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    let subscriber_id = match insert_subscriber(&pool, &new_subscriber).await {
        Ok(subscriber_id) => subscriber_id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let subscription_token = generate_subscription_token();
    if store_token(&pool, subscriber_id ,&subscription_token).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    // Send a (useless) email to the new subscriber.
    // We are ignoring email delivery errors for now.
    if send_confirmation_email(&email_client,
                               new_subscriber,
                               &base_url.get_ref().0,
                               // new parameter
                               &subscription_token,
    )
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }

    // handle 'ok' and 'err' paths
    HttpResponse::Ok().finish()
}

#[tracing::instrument (
name = "Send a confirmation email to a new subscriber",
skip(email_client, new_subscriber, base_url, subscription_token),
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    // new parameter
    subscription_token: &str
) -> Result<(), reqwest::Error>{
    let confirmation_link = format!("{}/subscriptions/confirm?subscription_token={}",
                                    base_url,
                                    subscription_token);
    let plain_body = format!("Welcome to the newsletter!\n
             Visit {} to confirm your subscription.",
                              confirmation_link);
    let html_body = format!("Welcome to the newsletter! <br />\
            Click <a href=\"{}\">here</a> to confirm your subscription.",
                             confirmation_link);

    email_client
        .send_email(
            new_subscriber.email,
            "Welcome!",
            &html_body,
            &plain_body,
        )
        .await
}

//Returns 'true' if the input satisfies all our validation constraints
//on subscriber names, 'false' otherwise
pub fn is_valid_name(s: &str) -> bool {
    // '.trim()' returns a view over the input 's' without trailing
    // whitespace-like characters.
    // '.is_empty' checks if the view contains any character.
    let is_empty_or_whitespace = s.trim().is_empty();

    // A grapheme is deifined by the Unicode standard as a "user-perceived"
    // character: 'å' is a single grapheme, but it is composed of two characters
    // ('a' and 'º).
    //
    // 'graphemes' returns an iterator over the graphemes in the input 's'.
    // 'true' specifies that we want to use the extended grapheme definition set,
    // the recommended one.
    let is_too_long = s.graphemes(true).count() > 256;

    //Iterate over all characters in the input 's' to check if any of them matches
    // one of the chracters in the forbidden array.
    let forbidden_characters = ['/', '(', ')', '"', '<','>', '\\', '{', '}'];
    let contains_forbidden_characters = s
        .chars()
        .any(|g| forbidden_characters.contains(&g));

    //Return 'false' if any of our conditions have been violated
    !(is_empty_or_whitespace || is_too_long || contains_forbidden_characters)
}

//Take care of database logic
#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(new_subscriber, pool)
)]
pub async fn insert_subscriber(
    pool: &PgPool,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id,email,name, subscribed_at, status)
        VALUES ($1,$2,$3,$4,'pending_confirmation')
        "#,
        // this subscriber id never returned or bound to a variable sadly. refactor it
        subscriber_id,
        new_subscriber.email.as_ref(),
        // using 'inner_ref'!
        new_subscriber.name.as_ref(),
        Utc::now()
        )
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to execute query: {:?}", e);
            e
        // Using the '?' operator to return early
        // if the function failed, returning a sqlx::Error
        // We will talk about error handling in depth later!
        })?;
    Ok(subscriber_id)
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        // take 25 characters = 10*45 possible tokens
        .take(25)
        .collect()
}


//Take care of database logic
#[tracing::instrument(
name = "Store subscription token in the database",
skip(subscription_token, pool)
)]
pub async fn store_token( pool: &PgPool,
                                subscriber_id: Uuid,
                                subscription_token: &str,
) -> Result<(), sqlx::Error>{
    sqlx::query!(
        r#"
        INSERT INTO subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1,$2)
        "#,
        subscription_token,
        subscriber_id,
        )
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to execute query: {:?}", e);
            e
            // Using the '?' operator to return early
            // if the function failed, returning a sqlx::Error
            // We will talk about error handling in depth later!
        })?;
    Ok(())
}
