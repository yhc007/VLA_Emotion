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

// ─── Error ───

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}
