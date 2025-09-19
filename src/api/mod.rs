pub mod handlers;
pub mod middleware;
pub mod state;
pub mod routes;
pub mod template_handler;
pub mod error;

pub use state::ApiState;
pub use routes::configure_routes;
pub use error::{ApiError, ApiResult};