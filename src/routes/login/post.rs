use actix_web::HttpResponse;
use actix_web::http::header::LOCATION;
use actix_web::{web, ResponseError};
use secrecy::Secret;
use crate::authentication::{validate_credentials, Credentials, AuthError};
use sqlx::PgPool;
use crate::routes::error_chain_fmt;
use actix_web::http::StatusCode;
use hmac::{Hmac, Mac};
use secrecy::ExposeSecret;
use actix_web::error::InternalError;
use crate::startup::HmacSecret;
use actix_web::cookie::Cookie;
use actix_web_flash_messages::FlashMessage;
use actix_session::Session;
use crate::session_state::TypedSession;

#[derive(serde::Deserialize)]
pub struct FormData {
    username: String,
    password: Secret<String>,
}

//extract authentication module to use in our login function
#[tracing::instrument(
    skip(form, pool, session),
    fields(username=tracing::field::Empty, user_id=tracing::field::Empty)
)]
// We are now injecting 'PgPool' to retrieve stored credentials from the database
pub async fn login(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    //injecting the secret as a secret string for the time being.
    //inject the wrapper type
    //dont need hmacsecret anymore
    //secret: web::Data<HmacSecret>,
    session: TypedSession,
) -> //Result<HttpResponse, LoginError> {
Result<HttpResponse, InternalError<LoginError>> {
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };
    tracing::Span::current()
        .record("username", &tracing::field::display(&credentials.username));
    match validate_credentials(credentials, &pool).await {
        Ok(user_id) => {
            tracing::Span::current()
                .record("user_id", &tracing::field::display(&user_id));
            session.renew();
            session.insert_user_id( user_id)
                .map_err(|e| login_redirect(LoginError::UnexpectedError(e.into())))?;
            Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/admin/dashboard"))
                .finish())
        }
        Err(e) => {
            let e = match e {
            AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
            AuthError::UnexpectedError(_) => {
              LoginError::UnexpectedError(e.into())
            },
            };
            /*
            let query_string = format!(
                "error={}",
                urlencoding::Encoded::new(e.to_string())
            );
            let hmac_tag= {
                let mut mac = Hmac::<sha2::Sha256>::new_from_slice(
                        secret.0.expose_secret().as_bytes()).unwrap();
                mac.update(query_string.as_bytes());
                mac.finalize().into_bytes()
            };
            */

            Err(login_redirect(e))
        }
}
    /* V1 implemenetation pre-secret implementation
    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };
    tracing::Span::current()
        .record("username", &tracing::field::display(&credentials.username));
    let user_id = validate_credentials(credentials, &pool).await
        .map_err(|e| match e {
            AuthError::InvalidCredentials(_) => LoginError::AuthError(e.into()),
            AuthError::UnexpectedError(_) => LoginError::UnexpectedError(e.into()),
        })?;

        tracing::Span::current()
            .record("user_id", &tracing::field::display(&user_id));
        Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/"))
            .finish())

     */
}

fn login_redirect(e: LoginError) -> InternalError<LoginError> {
    FlashMessage::error(e.to_string()).send();
    let response = HttpResponse::SeeOther()
        .insert_header((
            LOCATION, "/login"))
        //.insert_header(("Set-Cookie", format!("_flash={e}")))
        .finish();
    InternalError::from_response(e, response)

}

#[derive(thiserror::Error)]
pub enum LoginError{
    #[error("Authentication failed")]
    AuthError(#[source] anyhow::Error),
    #[error("Something went wrong")]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for LoginError {
    fn status_code(&self) -> StatusCode {
        StatusCode::SEE_OTHER
    }
    /*
    fn error_response(&self) -> HttpResponse {
        let query_string = format!("error={}", urlencoding::Encoded::new(self.to_string())
        );
        // we need the secret here- how do we get it?
        let secret: &[u8] = todo!();
        let hmac_tag = {
            let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret).unwrap();
            mac.update(query_string.as_bytes());
            mac.finalize().into_bytes()
        };

        HttpResponse::build(self.status_code())
        // appending the hexadecimal respresnetation of the HMAC tag to the
        // query string as an additional query parameter.
            .insert_header((
                    LOCATION,
                format!("/login?{query_string}&tag={hmac_tag:x}")
                ))
            .finish()
        /*
        let encoded_error = urlencoding::Encoded::new(self.to_string());
        HttpResponse::build(self.status_code())
            .insert_header((LOCATION, format!("/login?error={}", encoded_error)))
            .finish()
        */
    }
    */

}