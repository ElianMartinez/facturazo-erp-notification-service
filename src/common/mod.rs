//! Common utilities and helpers
//!
//! Shared functionality used across different layers

pub mod middleware;
pub mod security;
pub mod utils;

/// Common result type
pub type Result<T> = anyhow::Result<T>;

/// Common error type
pub use anyhow::Error;