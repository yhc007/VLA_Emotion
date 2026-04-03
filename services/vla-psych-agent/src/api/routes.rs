//! API Routes

use axum::{
    Router,
    routing::{get, post},
};
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;

use super::handlers::{
    health_check, start_session, analyze_text, analyze_audio, analyze_face,
    get_session_state, get_session_graph, generate_report, suggest_counselor_response,
};
use super::state::AppState;
use super::websocket::{ws_handler, ws_session_handler};

/// 메인 라우터 생성
pub fn create_router(state: AppState) -> Router {
    // CORS 설정
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health check
        .route("/health", get(health_check))
        
        // Session management
        .route("/api/v1/session/start", post(start_session))
        .route("/api/v1/session/:id/state", get(get_session_state))
        .route("/api/v1/session/:id/graph", get(get_session_graph))
        
        // Analysis
        .route("/api/v1/analyze/text", post(analyze_text))
        .route("/api/v1/analyze/audio", post(analyze_audio))
        .route("/api/v1/analyze/face", post(analyze_face))
        
        // LLM-powered
        .route("/api/v1/report/generate", post(generate_report))
        .route("/api/v1/counselor/suggest", post(suggest_counselor_response))
        
        // WebSocket (실시간 스트리밍)
        .route("/ws", get(ws_handler))
        .route("/ws/:id", get(ws_session_handler))
        
        // Middleware
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
