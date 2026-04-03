use serde::{Deserialize, Serialize};

// ============================================
// Case
// ============================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Case {
    pub id: String,
    pub client_id: String,
    pub analyst_id: String,
    pub status: CaseStatus,
    pub media_set: MediaSet,
    pub solution: Option<SolutionReport>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CaseStatus {
    Draft,
    Active,
    Analyzing,
    Completed,
    Archived,
}

// ============================================
// MediaSet
// ============================================

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MediaSet {
    pub id: String,
    pub case_id: String,
    pub video: Option<VideoStream>,
    pub audio: Option<AudioStream>,
    pub documents: Vec<Document>,
    pub timeline: Timeline,
}

// ============================================
// Video
// ============================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoStream {
    pub id: String,
    pub url: Option<String>,
    pub duration_ms: u64,
    pub frames: Vec<FrameAnalysis>,
    pub status: StreamStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrameAnalysis {
    pub timestamp_ms: u64,
    pub sentiment: String,
    pub confidence: f64,
    pub face_detected: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StreamStatus {
    Idle,
    Recording,
    Processing,
    Ready,
    Error,
}

// ============================================
// Audio
// ============================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioStream {
    pub id: String,
    pub url: Option<String>,
    pub duration_ms: u64,
    pub transcript: Vec<TranscriptSegment>,
    pub status: StreamStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Speaker,
    pub text: String,
    pub confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    Client,
    Analyst,
    Unknown,
}

// ============================================
// Document
// ============================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub filename: String,
    pub file_type: DocType,
    pub url: String,
    pub size_bytes: u64,
    pub extracted_text: Option<String>,
    pub mentioned_at: Vec<String>,  // TimeMarker IDs
    pub uploaded_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DocType {
    Pdf,
    Image,
    Excel,
    Word,
    Text,
    Other,
}

impl DocType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "pdf" => DocType::Pdf,
            "jpg" | "jpeg" | "png" | "gif" | "webp" => DocType::Image,
            "xlsx" | "xls" => DocType::Excel,
            "docx" | "doc" => DocType::Word,
            "txt" | "md" => DocType::Text,
            _ => DocType::Other,
        }
    }
    
    pub fn icon(&self) -> &'static str {
        match self {
            DocType::Pdf => "📄",
            DocType::Image => "🖼️",
            DocType::Excel => "📊",
            DocType::Word => "📝",
            DocType::Text => "📃",
            DocType::Other => "📁",
        }
    }
}

// ============================================
// Timeline
// ============================================

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Timeline {
    pub markers: Vec<TimeMarker>,
    pub events: Vec<TimelineEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeMarker {
    pub id: String,
    pub timestamp_ms: u64,
    pub media_type: MediaType,
    pub media_id: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub timestamp_ms: u64,
    pub event_type: EventType,
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Video,
    Audio,
    Document,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    SentimentChange,
    DocumentMention,
    KeyInsight,
    ProblemIdentified,
    SolutionProposed,
}

// ============================================
// Solution Report
// ============================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolutionReport {
    pub id: String,
    pub case_id: String,
    pub problem_statement: String,
    pub root_causes: Vec<String>,
    pub priority: Priority,
    pub solutions: Vec<Solution>,
    pub action_items: Vec<ActionItem>,
    pub next_steps: String,
    pub generated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Solution {
    pub id: String,
    pub title: String,
    pub description: String,
    pub effort: Effort,
    pub impact: Impact,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionItem {
    pub id: String,
    pub description: String,
    pub assignee: Option<String>,
    pub due_date: Option<String>,
    pub status: ActionStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Impact {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ActionStatus {
    Pending,
    InProgress,
    Completed,
}
