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

#[derive(Debug, Serialize, Deserialize)]
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

// ─── Face Analysis ───

#[derive(Debug, Serialize)]
pub struct FaceAnalyzeResponse {
    pub session_id: Uuid,
    /// 감지된 얼굴 수
    pub face_count: usize,
    /// 각 얼굴의 표정 분석
    pub faces: Vec<FaceDto>,
    /// 주요 감정
    pub primary_emotion: Option<String>,
    /// 주요 감정의 valence
    pub primary_valence: f32,
    /// 주요 감정의 arousal
    pub primary_arousal: f32,
    /// VLA 분석에 사용할 수 있는 EmotionInput 형식
    pub emotion_input: Option<EmotionInput>,
}

#[derive(Debug, Serialize)]
pub struct FaceDto {
    pub face_id: String,
    pub bounding_box: BoundingBoxDto,
    pub emotion: String,
    pub valence: f32,
    pub arousal: f32,
    pub confidence: f32,
}

#[derive(Debug, Serialize)]
pub struct BoundingBoxDto {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

// ─── LLM Report ───

#[derive(Debug, Deserialize)]
pub struct GenerateReportRequest {
    pub session_id: Uuid,
    pub conversation_summary: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PsychReportResponse {
    pub session_id: Uuid,
    pub report: crate::llm::PsychReport,
}

#[derive(Debug, Deserialize)]
pub struct CounselorSuggestRequest {
    pub session_id: Uuid,
    pub last_utterance: String,
    pub context: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CounselorSuggestionResponse {
    pub session_id: Uuid,
    pub suggestion: crate::llm::CounselorSuggestion,
}

// ─── Error ───

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}
