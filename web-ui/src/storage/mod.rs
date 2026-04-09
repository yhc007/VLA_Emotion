use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// 세션 메타데이터
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionMetadata {
    pub session_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub participant_count: usize,
    pub total_utterances: usize,
    pub total_duration_seconds: f64,
    pub languages: Vec<String>,
    pub keywords: Vec<String>,
    pub sentiment_summary: SentimentSummary,
    pub problem_categories: Vec<String>,
}

/// 감정 요약 통계
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SentimentSummary {
    pub dominant_sentiment: String,
    pub average_confidence: f64,
    pub sentiment_distribution: HashMap<String, f64>,
    pub emotional_volatility: f64,
}

/// Raw 대화 데이터
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RawConversationData {
    pub session_metadata: SessionMetadata,
    pub transcript_entries: Vec<TranscriptEntryWithTimestamp>,
    pub sentiment_data: Vec<SentimentDataWithTimestamp>,
    pub reasoning_results: Vec<ReasoningResultWithTimestamp>,
    pub audio_segments: Vec<AudioSegmentInfo>,
}

/// 타임스탬프가 포함된 대화 엔트리
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TranscriptEntryWithTimestamp {
    pub timestamp: DateTime<Utc>,
    pub text: String,
    pub speaker: String,
    pub confidence: Option<f64>,
    pub language: Option<String>,
    pub duration_ms: Option<u64>,
}

/// 타임스탬프가 포함된 감정 데이터
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SentimentDataWithTimestamp {
    pub timestamp: DateTime<Utc>,
    pub sentiment: String,
    pub confidence: f64,
    pub text_analyzed: String,
}

/// 타임스탬프가 포함된 추론 결과
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReasoningResultWithTimestamp {
    pub timestamp: DateTime<Utc>,
    pub reasoning_result: String, // JSON으로 직렬화된 ReasoningResult
    pub trigger_text: String,
}

/// 오디오 세그먼트 정보
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AudioSegmentInfo {
    pub start_time: DateTime<Utc>,
    pub duration_ms: u64,
    pub file_path: Option<String>, // 실제 오디오 파일 경로 (선택적)
    pub speaker: String,
    pub quality_score: Option<f64>,
}

/// 처리된 분석 데이터
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProcessedAnalysisData {
    pub session_id: String,
    pub analysis_timestamp: DateTime<Utc>,
    pub problem_classification: ProblemClassification,
    pub emotional_journey: Vec<EmotionalState>,
    pub key_insights: Vec<String>,
    pub recommendations: Vec<String>,
    pub risk_factors: Vec<RiskFactor>,
}

/// 문제 분류
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProblemClassification {
    pub primary_category: String,
    pub secondary_categories: Vec<String>,
    pub severity_level: String, // "low", "medium", "high", "critical"
    pub confidence: f64,
}

/// 감정 상태 추적
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EmotionalState {
    pub timestamp: DateTime<Utc>,
    pub emotion: String,
    pub intensity: f64, // 0.0 ~ 1.0
    pub trigger: Option<String>,
    pub context: Option<String>,
}

/// 위험 요소
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RiskFactor {
    pub factor_type: String,
    pub description: String,
    pub severity: String,
    pub evidence: Vec<String>,
}

/// 저장 관리자 (WASM/브라우저용)
pub struct DataStorageManager {
    server_url: String,
}

impl DataStorageManager {
    pub fn new(server_url: &str) -> Self {
        Self {
            server_url: server_url.to_string(),
        }
    }

    /// 새 세션 시작 (서버에 요청)
    pub async fn start_session(&self, session_id: &str) -> Result<(), String> {
        use gloo_net::http::Request;
        
        let metadata = SessionMetadata {
            session_id: session_id.to_string(),
            start_time: Utc::now(),
            end_time: None,
            participant_count: 0,
            total_utterances: 0,
            total_duration_seconds: 0.0,
            languages: vec!["ko-KR".to_string()],
            keywords: vec![],
            sentiment_summary: SentimentSummary {
                dominant_sentiment: "neutral".to_string(),
                average_confidence: 0.0,
                sentiment_distribution: HashMap::new(),
                emotional_volatility: 0.0,
            },
            problem_categories: vec![],
        };
        
        // 서버로 세션 시작 요청
        let url = format!("{}/api/storage/sessions", self.server_url);
        let response = Request::post(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&metadata).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| format!("Failed to start session: {}", e))?;
        
        if response.ok() {
            log::info!("Session {} started successfully", session_id);
            Ok(())
        } else {
            Err(format!("Server error: {}", response.status()))
        }
    }

    /// 대화 엔트리 추가 (서버로 전송)
    pub async fn append_transcript(&self, session_id: &str, text: &str, speaker: &str) -> Result<(), String> {
        use gloo_net::http::Request;
        
        let entry = TranscriptEntryWithTimestamp {
            timestamp: Utc::now(),
            text: text.to_string(),
            speaker: speaker.to_string(),
            confidence: None,
            language: Some("ko-KR".to_string()),
            duration_ms: None,
        };
        
        let url = format!("{}/api/storage/sessions/{}/transcript", self.server_url, session_id);
        let response = Request::post(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&entry).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| format!("Failed to append transcript: {}", e))?;
        
        if response.ok() {
            log::debug!("Transcript appended: {} - {}", speaker, text);
            Ok(())
        } else {
            Err(format!("Server error: {}", response.status()))
        }
    }

    /// 감정 분석 결과 저장 (서버로 전송)
    pub async fn save_sentiment_analysis(&self, session_id: &str, sentiment: &str, confidence: f64, text: &str) -> Result<(), String> {
        use gloo_net::http::Request;
        
        let entry = SentimentDataWithTimestamp {
            timestamp: Utc::now(),
            sentiment: sentiment.to_string(),
            confidence,
            text_analyzed: text.to_string(),
        };
        
        let url = format!("{}/api/storage/sessions/{}/sentiment", self.server_url, session_id);
        let response = Request::post(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&entry).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| format!("Failed to save sentiment: {}", e))?;
        
        if response.ok() {
            log::debug!("Sentiment saved: {} ({})", sentiment, confidence);
            Ok(())
        } else {
            Err(format!("Server error: {}", response.status()))
        }
    }

    /// 추론 결과 저장 (서버로 전송)
    pub async fn save_reasoning_result(&self, session_id: &str, reasoning_json: &str, trigger_text: &str) -> Result<(), String> {
        use gloo_net::http::Request;
        
        let entry = ReasoningResultWithTimestamp {
            timestamp: Utc::now(),
            reasoning_result: reasoning_json.to_string(),
            trigger_text: trigger_text.to_string(),
        };
        
        let url = format!("{}/api/storage/sessions/{}/reasoning", self.server_url, session_id);
        let response = Request::post(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(&entry).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| format!("Failed to save reasoning: {}", e))?;
        
        if response.ok() {
            log::debug!("Reasoning saved for trigger: {}", trigger_text);
            Ok(())
        } else {
            Err(format!("Server error: {}", response.status()))
        }
    }

    /// 세션 메타데이터 저장 (서버로 전송)
    pub async fn save_session_metadata(&self, metadata: &SessionMetadata) -> Result<(), String> {
        use gloo_net::http::Request;
        
        let url = format!("{}/api/storage/sessions/{}/metadata", self.server_url, metadata.session_id);
        let response = Request::put(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(metadata).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| format!("Failed to save metadata: {}", e))?;
        
        if response.ok() {
            log::debug!("Metadata saved for session: {}", metadata.session_id);
            Ok(())
        } else {
            Err(format!("Server error: {}", response.status()))
        }
    }

    /// 세션 종료 (서버에 종료 신호)
    pub async fn end_session(&self, session_id: &str) -> Result<(), String> {
        use gloo_net::http::Request;
        
        let url = format!("{}/api/storage/sessions/{}/end", self.server_url, session_id);
        let response = Request::post(&url)
            .header("Content-Type", "application/json")
            .body(serde_json::json!({ "end_time": Utc::now() }).to_string())
            .map_err(|e| e.to_string())?
            .send()
            .await
            .map_err(|e| format!("Failed to end session: {}", e))?;
        
        if response.ok() {
            log::info!("Session {} ended successfully", session_id);
            Ok(())
        } else {
            Err(format!("Server error: {}", response.status()))
        }
    }
}

/// 전역 저장소 인스턴스 (웹에서 사용)
pub static mut STORAGE_MANAGER: Option<DataStorageManager> = None;

/// 저장소 초기화 (서버 URL 설정)
pub fn initialize_storage(server_url: &str) {
    unsafe {
        STORAGE_MANAGER = Some(DataStorageManager::new(server_url));
    }
}

/// 저장소 접근
pub fn get_storage() -> Option<&'static DataStorageManager> {
    unsafe { STORAGE_MANAGER.as_ref() }
}