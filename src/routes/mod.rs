// When crate::routes is called it searches for a mod.rs file definition.
// Similar to an index.js file.

mod health_check;
mod subscriptions;

pub use health_check::*;
pub use subscriptions::*;
