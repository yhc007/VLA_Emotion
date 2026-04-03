//! API Handlers

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::types::EmotionResult;
use super::dto::*;
use super::state::AppState;

/// GET /health
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "vla-psych-agent".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// POST /api/v1/session/start
pub async fn start_session(
    State(state): State<AppState>,
) -> Json<StartSessionResponse> {
    let session_id = state.create_session().await;
    
    info!(session_id = %session_id, "새 세션 시작");
    
    Json(StartSessionResponse {
        session_id,
        message: "세션이 시작되었습니다".to_string(),
    })
}

/// POST /api/v1/analyze/text
pub async fn analyze_text(
    State(state): State<AppState>,
    Json(req): Json<AnalyzeTextRequest>,
) -> Result<Json<AnalyzeTextResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 세션 확인
    let mut agent = match state.get_session(&req.session_id).await {
        Some(agent) => agent,
        None => {
            warn!(session_id = %req.session_id, "세션을 찾을 수 없음");
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "SESSION_NOT_FOUND".to_string(),
                    message: format!("세션 {}를 찾을 수 없습니다", req.session_id),
                }),
            ));
        }
    };

    // 감정 데이터 변환
    let emotion = req.emotion.map(|e| {
        EmotionResult::new(
            e.to_emotion_type(),
            e.valence,
            e.arousal,
            "api_input",
            req.session_id,
        )
    });

    // VLA 파이프라인 실행
    info!(
        session_id = %req.session_id,
        text = %req.text,
        has_emotion = emotion.is_some(),
        "텍스트 분석 시작"
    );

    let result = agent.process_text_input(&req.text, emotion);

    // 세션 업데이트
    state.update_session(&req.session_id, agent).await;

    let coherence_score = result.psych_state.coherence_score;
    
    Ok(Json(AnalyzeTextResponse {
        session_id: req.session_id,
        triples: result.triples,
        psych_state: result.psych_state,
        coherence_score,
        contradiction_detected: result.contradiction_detected,
        alerts: result.alerts,
        graph_summary: result.graph_summary,
    }))
}

/// GET /api/v1/session/:id/state
pub async fn get_session_state(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponse>, (StatusCode, Json<ErrorResponse>)> {
    let agent = match state.get_session(&session_id).await {
        Some(agent) => agent,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "SESSION_NOT_FOUND".to_string(),
                    message: format!("세션 {}를 찾을 수 없습니다", session_id),
                }),
            ));
        }
    };

    let (psych_state, utterance_count, node_count, edge_count) = agent.get_session_stats();

    Ok(Json(SessionStateResponse {
        session_id,
        psych_state,
        utterance_count,
        graph_node_count: node_count,
        graph_edge_count: edge_count,
    }))
}

/// GET /api/v1/session/:id/graph
pub async fn get_session_graph(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<GraphResponse>, (StatusCode, Json<ErrorResponse>)> {
    let agent = match state.get_session(&session_id).await {
        Some(agent) => agent,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "SESSION_NOT_FOUND".to_string(),
                    message: format!("세션 {}를 찾을 수 없습니다", session_id),
                }),
            ));
        }
    };

    let (nodes, edges) = agent.get_graph_data();

    Ok(Json(GraphResponse {
        session_id,
        nodes: nodes.into_iter().map(|(id, label, node_type)| GraphNode {
            id,
            label,
            node_type,
        }).collect(),
        edges: edges.into_iter().map(|(source, target, label, weight)| GraphEdge {
            source,
            target,
            label,
            weight,
        }).collect(),
    }))
}
