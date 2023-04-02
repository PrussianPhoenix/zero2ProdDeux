// When crate::routes is called it searches for a mod.rs file definition.
// Similar to an index.js file.

mod health_check;
mod subscriptions;
//new module
mod subscriptions_confirm;

mod home;
mod login;
mod admin;

pub use health_check::*;
pub use subscriptions::*;
pub use subscriptions_confirm::*;

pub use home::*;
pub use login::*;
pub use admin::*;