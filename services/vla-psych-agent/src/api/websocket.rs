//! WebSocket 실시간 스트리밍
//!
//! 클라이언트가 세션에 연결하여 실시간으로 분석 결과를 받음

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::types::EmotionResult;
use super::state::AppState;

/// WebSocket 메시지 타입 (클라이언트 → 서버)
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum WsClientMessage {
    /// 텍스트 분석 요청
    #[serde(rename = "analyze")]
    Analyze {
        text: String,
        #[serde(default)]
        emotion: Option<EmotionInput>,
    },
    /// 세션 상태 요청
    #[serde(rename = "get_state")]
    GetState,
    /// 그래프 데이터 요청
    #[serde(rename = "get_graph")]
    GetGraph,
    /// Ping (연결 유지)
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Debug, Deserialize)]
pub struct EmotionInput {
    pub emotion_type: String,
    pub valence: f32,
    pub arousal: f32,
}

/// WebSocket 메시지 타입 (서버 → 클라이언트)
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum WsServerMessage {
    /// 연결 성공
    #[serde(rename = "connected")]
    Connected { session_id: Uuid },
    /// 분석 결과
    #[serde(rename = "analysis_result")]
    AnalysisResult {
        triples_count: usize,
        psych_state: String,
        coherence_score: f32,
        contradiction_detected: bool,
        alerts_count: usize,
        graph_summary: String,
    },
    /// 세션 상태
    #[serde(rename = "session_state")]
    SessionState {
        psych_state: String,
        utterance_count: usize,
        graph_nodes: usize,
        graph_edges: usize,
    },
    /// 그래프 데이터
    #[serde(rename = "graph_data")]
    GraphData {
        nodes: Vec<GraphNodeDto>,
        edges: Vec<GraphEdgeDto>,
    },
    /// 이상 알림 (실시간 푸시)
    #[serde(rename = "alert")]
    Alert {
        alert_type: String,
        severity: String,
        message: String,
    },
    /// Pong
    #[serde(rename = "pong")]
    Pong,
    /// 에러
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Serialize)]
pub struct GraphNodeDto {
    pub id: String,
    pub label: String,
    pub node_type: String,
}

#[derive(Debug, Serialize)]
pub struct GraphEdgeDto {
    pub source: String,
    pub target: String,
    pub label: String,
}

/// WebSocket 업그레이드 핸들러
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// WebSocket 세션별 업그레이드 핸들러
pub async fn ws_session_handler(
    ws: WebSocketUpgrade,
    Path(session_id): Path<Uuid>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket_with_session(socket, state, session_id))
}

/// 새 세션으로 WebSocket 처리
async fn handle_socket(socket: WebSocket, state: AppState) {
    // 새 세션 생성
    let session_id = state.create_session().await;
    info!(session_id = %session_id, "WebSocket 연결 (새 세션)");
    
    process_socket(socket, state, session_id).await;
}

/// 기존 세션으로 WebSocket 처리
async fn handle_socket_with_session(socket: WebSocket, state: AppState, session_id: Uuid) {
    if !state.session_exists(&session_id).await {
        warn!(session_id = %session_id, "존재하지 않는 세션");
        // 세션이 없으면 새로 생성
        let _ = state.create_session().await;
    }
    
    info!(session_id = %session_id, "WebSocket 연결 (기존 세션)");
    process_socket(socket, state, session_id).await;
}

/// WebSocket 메시지 처리 루프
async fn process_socket(socket: WebSocket, state: AppState, session_id: Uuid) {
    let (mut sender, mut receiver) = socket.split();

    // 연결 성공 메시지 전송
    let connected_msg = WsServerMessage::Connected { session_id };
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        let _ = sender.send(Message::Text(json)).await;
    }

    // 메시지 수신 루프
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let response = handle_client_message(&text, &state, &session_id).await;
                if let Ok(json) = serde_json::to_string(&response) {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
            }
            Ok(Message::Close(_)) => {
                info!(session_id = %session_id, "WebSocket 연결 종료");
                break;
            }
            Ok(Message::Ping(data)) => {
                let _ = sender.send(Message::Pong(data)).await;
            }
            Err(e) => {
                error!(session_id = %session_id, error = %e, "WebSocket 에러");
                break;
            }
            _ => {}
        }
    }
}

/// 클라이언트 메시지 처리
async fn handle_client_message(
    text: &str,
    state: &AppState,
    session_id: &Uuid,
) -> WsServerMessage {
    // JSON 파싱
    let client_msg: WsClientMessage = match serde_json::from_str(text) {
        Ok(msg) => msg,
        Err(e) => {
            return WsServerMessage::Error {
                message: format!("잘못된 메시지 형식: {}", e),
            };
        }
    };

    match client_msg {
        WsClientMessage::Analyze { text, emotion } => {
            handle_analyze(state, session_id, &text, emotion).await
        }
        WsClientMessage::GetState => {
            handle_get_state(state, session_id).await
        }
        WsClientMessage::GetGraph => {
            handle_get_graph(state, session_id).await
        }
        WsClientMessage::Ping => WsServerMessage::Pong,
    }
}

/// 텍스트 분석 처리
async fn handle_analyze(
    state: &AppState,
    session_id: &Uuid,
    text: &str,
    emotion_input: Option<EmotionInput>,
) -> WsServerMessage {
    let mut agent = match state.get_session(session_id).await {
        Some(agent) => agent,
        None => {
            return WsServerMessage::Error {
                message: "세션을 찾을 수 없습니다".to_string(),
            };
        }
    };

    // 감정 변환
    let emotion = emotion_input.map(|e| {
        use crate::types::EmotionType;
        let emo_type = match e.emotion_type.as_str() {
            "happy" => EmotionType::Happy,
            "sad" => EmotionType::Sad,
            "angry" => EmotionType::Angry,
            "surprised" => EmotionType::Surprised,
            "fearful" => EmotionType::Fearful,
            "disgusted" => EmotionType::Disgusted,
            "contempt" => EmotionType::Contempt,
            _ => EmotionType::Neutral,
        };
        EmotionResult::new(emo_type, e.valence, e.arousal, "ws_client", *session_id)
    });

    // VLA 파이프라인 실행
    let result = agent.process_text_input(text, emotion);

    // 세션 업데이트
    state.update_session(session_id, agent).await;

    WsServerMessage::AnalysisResult {
        triples_count: result.triples.len(),
        psych_state: format!("{:?}", result.psych_state.overall_state),
        coherence_score: result.psych_state.coherence_score,
        contradiction_detected: result.contradiction_detected,
        alerts_count: result.alerts.len(),
        graph_summary: result.graph_summary,
    }
}

/// 세션 상태 조회
async fn handle_get_state(state: &AppState, session_id: &Uuid) -> WsServerMessage {
    let agent = match state.get_session(session_id).await {
        Some(agent) => agent,
        None => {
            return WsServerMessage::Error {
                message: "세션을 찾을 수 없습니다".to_string(),
            };
        }
    };

    let (psych_state, utterance_count, node_count, edge_count) = agent.get_session_stats();

    WsServerMessage::SessionState {
        psych_state: format!("{:?}", psych_state.overall_state),
        utterance_count,
        graph_nodes: node_count,
        graph_edges: edge_count,
    }
}

/// 그래프 데이터 조회
async fn handle_get_graph(state: &AppState, session_id: &Uuid) -> WsServerMessage {
    let agent = match state.get_session(session_id).await {
        Some(agent) => agent,
        None => {
            return WsServerMessage::Error {
                message: "세션을 찾을 수 없습니다".to_string(),
            };
        }
    };

    let (nodes, edges) = agent.get_graph_data();

    WsServerMessage::GraphData {
        nodes: nodes.into_iter().map(|(id, label, node_type)| GraphNodeDto {
            id,
            label,
            node_type,
        }).collect(),
        edges: edges.into_iter().map(|(source, target, label, _)| GraphEdgeDto {
            source,
            target,
            label,
        }).collect(),
    }
}
