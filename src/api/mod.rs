pub mod error;
pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod state;
pub mod template_handler;

pub use error::{ApiError, ApiResult};
pub use routes::configure_routes;
pub use state::ApiState;
