pub mod admin;
pub mod preclude;
pub mod service;

pub use admin::config as admin_config;
pub use service::config as service_config;

mod error;
mod response;
pub use error::{UniError, UniResult};
pub use response::{QueryParams, UniResponse};
