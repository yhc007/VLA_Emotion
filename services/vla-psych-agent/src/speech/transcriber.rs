//! Speech Transcriber Trait
//!
//! 음성 인식 백엔드의 공통 인터페이스

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// 음성 인식 에러
#[derive(Debug, Error)]
pub enum TranscriberError {
    #[error("API 호출 실패: {0}")]
    ApiError(String),
    
    #[error("오디오 형식 오류: {0}")]
    AudioFormatError(String),
    
    #[error("파일 읽기 실패: {0}")]
    FileError(String),
    
    #[error("인식 실패: {0}")]
    RecognitionError(String),
    
    #[error("설정 오류: {0}")]
    ConfigError(String),
}

/// 음성 인식 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptResult {
    /// 전체 텍스트
    pub text: String,
    /// 세그먼트 (타임스탬프 포함)
    pub segments: Vec<TranscriptSegment>,
    /// 인식 언어
    pub language: String,
    /// 전체 오디오 길이 (초)
    pub duration_secs: f32,
    /// 인식 신뢰도 (0.0 ~ 1.0)
    pub confidence: f32,
    /// 세션 ID
    pub session_id: Uuid,
    /// 생성 시간
    pub created_at: DateTime<Utc>,
}

impl TranscriptResult {
    pub fn new(text: &str, language: &str, duration_secs: f32, session_id: Uuid) -> Self {
        Self {
            text: text.to_string(),
            segments: Vec::new(),
            language: language.to_string(),
            duration_secs,
            confidence: 1.0,
            session_id,
            created_at: Utc::now(),
        }
    }

    pub fn with_segments(mut self, segments: Vec<TranscriptSegment>) -> Self {
        self.segments = segments;
        self
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }
}

/// 음성 인식 세그먼트 (타임스탬프 포함)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    /// 세그먼트 ID
    pub id: usize,
    /// 시작 시간 (초)
    pub start: f32,
    /// 종료 시간 (초)
    pub end: f32,
    /// 텍스트
    pub text: String,
    /// 신뢰도
    pub confidence: f32,
    /// 화자 ID (diarization 시)
    pub speaker_id: Option<String>,
}

impl TranscriptSegment {
    pub fn new(id: usize, start: f32, end: f32, text: &str) -> Self {
        Self {
            id,
            start,
            end,
            text: text.to_string(),
            confidence: 1.0,
            speaker_id: None,
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_speaker(mut self, speaker_id: &str) -> Self {
        self.speaker_id = Some(speaker_id.to_string());
        self
    }
}

/// 음성 인식 백엔드 트레이트
#[async_trait]
pub trait SpeechTranscriber: Send + Sync {
    /// 오디오 파일에서 텍스트 추출
    async fn transcribe_file(
        &self,
        file_path: &str,
        session_id: Uuid,
    ) -> Result<TranscriptResult, TranscriberError>;

    /// 오디오 바이트에서 텍스트 추출
    async fn transcribe_bytes(
        &self,
        audio_data: &[u8],
        format: AudioFormat,
        session_id: Uuid,
    ) -> Result<TranscriptResult, TranscriberError>;

    /// 백엔드 이름
    fn name(&self) -> &str;

    /// 지원 언어 목록
    fn supported_languages(&self) -> Vec<&str>;
}

/// 오디오 형식
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Mp3,
    Webm,
    Ogg,
    Flac,
    M4a,
}

impl AudioFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "wav" => Some(Self::Wav),
            "mp3" => Some(Self::Mp3),
            "webm" => Some(Self::Webm),
            "ogg" => Some(Self::Ogg),
            "flac" => Some(Self::Flac),
            "m4a" => Some(Self::M4a),
            _ => None,
        }
    }

    pub fn mime_type(&self) -> &str {
        match self {
            Self::Wav => "audio/wav",
            Self::Mp3 => "audio/mpeg",
            Self::Webm => "audio/webm",
            Self::Ogg => "audio/ogg",
            Self::Flac => "audio/flac",
            Self::M4a => "audio/mp4",
        }
    }
}
