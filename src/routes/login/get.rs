use std::time;
use actix_web::{HttpResponse, web};
use actix_web::http::header::ContentType;
use crate::startup::HmacSecret;
use hmac::{Hmac, Mac};
use secrecy::ExposeSecret;
use actix_web::HttpRequest;
use actix_web::cookie::{Cookie, time::Duration};
use actix_web_flash_messages::{IncomingFlashMessages, Level};
use std::fmt::Write;
use std::hash::Hasher;

//extract error from request handler for GET /login
#[derive(serde::Deserialize)]
pub struct QueryParams {
    error: String,
    tag: String,
}

impl QueryParams {
    fn verify(self, secret: &HmacSecret) -> Result<String, anyhow::Error> {
        let tag = hex::decode(self.tag)?;
        let query_string = format!(
            "error={}",
            urlencoding::Encoded::new(&self.error)
        );
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(
            secret.0.expose_secret().as_bytes()
        ).unwrap();
        mac.update(query_string.as_bytes());
        mac.verify_slice(&tag)?;

        Ok(self.error)
    }
}

//No need to access the raw request anymore!
pub async fn login_form(flash_messages: IncomingFlashMessages) -> HttpResponse {
    /*
    let _error = query.0.error;
    HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(include_str!("login.html"))
    */
    /*
    let error_html = match query {
        None => "".into(),
        Some(query) => match query.0.verify(&secret) {
            Ok(error) => {
                format!("<p><i>{}</i></p>", htmlescape::encode_minimal(&error))
            }
            Err(e) => {
                    tracing::warn!(
                        error.message = %e,
                        error.cause_chain = ?e,
                        "Failed to verify query parameters using the HMAC tag"
                    );
                "".into()
            }
        },
    };

     */

    /*
    let error_html = match request.cookie("_flash"){
        None => "".into(),
        Some(cookie) => {
            format!("<p><i>{}</i></p>", cookie.value())
        }
    };
    */

    let mut error_html = String::new();
    // display all messages, not just errors
    // errors = messeges.iter().filter(|m| m.level() == Level::Error)
    for m in flash_messages.iter() {
        writeln!(error_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    HttpResponse::Ok()
        .content_type(ContentType::html())
        /*
        .cookie(Cookie::build("_flash", "")
            .max_age(Duration::ZERO)
            .finish(),)
        */
        .body(format!(
            r#"<!DOCTYPE html>
            <html lang="en">
                <head>
                    <meta http-equiv="content-type content="text/html; charset=utf-8">
                    <title>Login</title>
                </head>
                <body>
                    {error_html}
                    <form action = "/login" method="post">
                        <label>Username
                            <input
                                type="text"
                                placeholder = "Enter Username"
                                name="username"
                            >
                        </label>
                        <label>Password
                            <input
                                type="password"
                                placeholder="Enter Password"
                                name="password"
                            >
                        </label>
                        <button type="submit">Login</button>
                    </form>
                </body>
                </html>"#,
        ))
    /*
    response.add_removal_cookie(&Cookie::new("_flash", ""))
        .unwrap();
    response
    */
}