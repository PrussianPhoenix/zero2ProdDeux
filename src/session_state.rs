use actix_session::{Session, SessionGetError, SessionInsertError};
use uuid::Uuid;
use actix_session::SessionExt;
use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest};
use std::future::{Ready, ready};


pub struct TypedSession(Session);

impl TypedSession {
    const USER_ID_KEY: &'static str = "user_id";

    pub fn renew(&self) {
        self.0.renew();
    }

    pub fn insert_user_id(&self, user_id: Uuid) -> Result<(), SessionInsertError> {
        self.0.insert(Self::USER_ID_KEY, user_id)
    }

    pub fn get_user_id(&self) -> Result<Option<Uuid>, SessionGetError> {
        self.0.get(Self::USER_ID_KEY)
    }
}

impl FromRequest for TypedSession {
    // this is a complicated way of saying
    // we return the same error returned by the implementation
    // of 'FromRequest' for 'Session'"
    type Error = <Session as FromRequest>::Error;
    // Rust does not yet support the 'async' syntax in traits
    // From request expects a 'Future', because we don't perform any I/O
    // so we wrap 'TypedSession' into 'Ready' to convert it into a 'Future' that
    // resolves to the wrapped value for the first time its polled by the executor.
    type Future = Ready<Result<TypedSession, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        ready(Ok(TypedSession(req.get_session())))
    }
}