//! VLA Psych Agent HTTP API
//!
//! REST API + WebSocket endpoints for VLA psychological analysis.

mod dto;
mod handlers;
mod routes;
mod state;
mod websocket;

pub use routes::create_router;
pub use state::AppState;
