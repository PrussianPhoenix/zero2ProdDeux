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

#[tracing::instrument (
    name = "Adding a new subscriber",
    skip(form, pool, email_client),
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
) -> HttpResponse {
    let new_subscriber = match form.0.try_into(){
        Ok(form) => form,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    if insert_subscriber(&pool, &new_subscriber).await.is_err()
    {
        // e handled by insert_subscriber
        return HttpResponse::InternalServerError().finish()
    }

    // Send a (useless) email to the new subscriber.
    // We are ignoring email delivery errors for now.
    if send_confirmation_email(&email_client, new_subscriber
    )
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish()
    }

    // handle 'ok' and 'err' paths
    return HttpResponse::Ok().finish()
}

#[tracing::instrument (
name = "Send a confirmation email to a new subcriber",
skip(email_client, new_subscriber),
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
) -> Result<(), reqwest::Error>{
    let confirmation_link = "https://there-is-no-such-domain.com/subscriptions/confirm";

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
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id,email,name, subscribed_at, status)
        VALUES ($1,$2,$3,$4,'pending_confirmation')
        "#,
        Uuid::new_v4(),
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
    Ok(())
}
