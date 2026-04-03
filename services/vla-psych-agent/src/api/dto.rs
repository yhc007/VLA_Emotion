//! API Request/Response DTOs

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{EmotionType, PsychState, SPOTriple, AnomalyAlert};

// ─── Session ───

#[derive(Debug, Serialize)]
pub struct StartSessionResponse {
    pub session_id: Uuid,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct SessionStateResponse {
    pub session_id: Uuid,
    pub psych_state: PsychState,
    pub utterance_count: usize,
    pub graph_node_count: usize,
    pub graph_edge_count: usize,
}

// ─── Text Analysis ───

#[derive(Debug, Deserialize)]
pub struct AnalyzeTextRequest {
    pub session_id: Uuid,
    pub text: String,
    #[serde(default)]
    pub emotion: Option<EmotionInput>,
}

#[derive(Debug, Deserialize)]
pub struct EmotionInput {
    pub emotion_type: String,  // "happy", "sad", "angry", etc.
    pub valence: f32,          // -1.0 ~ 1.0
    pub arousal: f32,          // 0.0 ~ 1.0
}

impl EmotionInput {
    pub fn to_emotion_type(&self) -> EmotionType {
        match self.emotion_type.to_lowercase().as_str() {
            "happy" => EmotionType::Happy,
            "sad" => EmotionType::Sad,
            "angry" => EmotionType::Angry,
            "surprised" => EmotionType::Surprised,
            "fearful" => EmotionType::Fearful,
            "disgusted" => EmotionType::Disgusted,
            "contempt" => EmotionType::Contempt,
            _ => EmotionType::Neutral,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AnalyzeTextResponse {
    pub session_id: Uuid,
    pub triples: Vec<SPOTriple>,
    pub psych_state: PsychState,
    pub coherence_score: f32,
    pub contradiction_detected: bool,
    pub alerts: Vec<AnomalyAlert>,
    pub graph_summary: String,
}

// ─── Graph ───

#[derive(Debug, Serialize)]
pub struct GraphResponse {
    pub session_id: Uuid,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String,  // "entity", "emotion", "state"
}

#[derive(Debug, Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub label: String,
    pub weight: f32,
}

// ─── Health ───

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

// ─── Audio Analysis ───

#[derive(Debug, Serialize)]
pub struct AudioAnalyzeResponse {
    pub session_id: Uuid,
    /// 음성 인식 결과 (전체 텍스트)
    pub transcript: String,
    /// 세그먼트 (타임스탬프 포함)
    pub transcript_segments: Vec<TranscriptSegmentDto>,
    /// 인식된 언어
    pub language: String,
    /// 오디오 길이 (초)
    pub duration_secs: f32,
    /// SPO 트리플
    pub triples: Vec<SPOTriple>,
    /// 심리 상태
    pub psych_state: PsychState,
    /// 일치도
    pub coherence_score: f32,
    /// 말-표정 불일치
    pub contradiction_detected: bool,
    /// 이상 알림
    pub alerts: Vec<AnomalyAlert>,
    /// 그래프 요약
    pub graph_summary: String,
}

#[derive(Debug, Serialize)]
pub struct TranscriptSegmentDto {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

// ─── Error ───

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}
