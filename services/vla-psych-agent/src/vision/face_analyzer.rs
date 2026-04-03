//! Face Analyzer Trait
//!
//! 표정 인식 백엔드의 공통 인터페이스

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::types::EmotionType;

/// 표정 분석 에러
#[derive(Debug, Error)]
pub enum FaceAnalyzerError {
    #[error("API 호출 실패: {0}")]
    ApiError(String),
    
    #[error("이미지 형식 오류: {0}")]
    ImageFormatError(String),
    
    #[error("파일 읽기 실패: {0}")]
    FileError(String),
    
    #[error("얼굴 감지 실패: {0}")]
    DetectionError(String),
    
    #[error("설정 오류: {0}")]
    ConfigError(String),
}

/// 표정 분석 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceAnalysisResult {
    /// 감지된 얼굴 수
    pub face_count: usize,
    /// 각 얼굴의 표정 분석
    pub faces: Vec<FaceEmotion>,
    /// 주요 감정 (가장 큰 얼굴 또는 첫 번째)
    pub primary_emotion: Option<EmotionType>,
    /// 주요 감정의 valence/arousal
    pub primary_valence: f32,
    pub primary_arousal: f32,
    /// 분석 시간
    pub timestamp: DateTime<Utc>,
    /// 세션 ID
    pub session_id: Uuid,
}

impl FaceAnalysisResult {
    pub fn empty(session_id: Uuid) -> Self {
        Self {
            face_count: 0,
            faces: Vec::new(),
            primary_emotion: None,
            primary_valence: 0.0,
            primary_arousal: 0.5,
            timestamp: Utc::now(),
            session_id,
        }
    }

    pub fn single(face: FaceEmotion, session_id: Uuid) -> Self {
        Self {
            face_count: 1,
            primary_emotion: Some(face.emotion.clone()),
            primary_valence: face.valence,
            primary_arousal: face.arousal,
            faces: vec![face],
            timestamp: Utc::now(),
            session_id,
        }
    }

    /// EmotionResult로 변환
    pub fn to_emotion_result(&self, speaker_id: &str) -> Option<crate::types::EmotionResult> {
        self.primary_emotion.as_ref().map(|emotion| {
            crate::types::EmotionResult::new(
                emotion.clone(),
                self.primary_valence,
                self.primary_arousal,
                speaker_id,
                self.session_id,
            )
        })
    }
}

/// 개별 얼굴의 표정 분석
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceEmotion {
    /// 얼굴 ID
    pub face_id: String,
    /// 바운딩 박스
    pub bounding_box: BoundingBox,
    /// 감지된 감정
    pub emotion: EmotionType,
    /// 감정별 신뢰도
    pub emotion_scores: EmotionScores,
    /// valence (-1.0 ~ 1.0)
    pub valence: f32,
    /// arousal (0.0 ~ 1.0)
    pub arousal: f32,
    /// 전체 신뢰도
    pub confidence: f32,
}

impl FaceEmotion {
    pub fn new(
        face_id: &str,
        bounding_box: BoundingBox,
        emotion: EmotionType,
        scores: EmotionScores,
    ) -> Self {
        let (valence, arousal) = scores.to_valence_arousal();
        let confidence = scores.max_score();
        
        Self {
            face_id: face_id.to_string(),
            bounding_box,
            emotion,
            emotion_scores: scores,
            valence,
            arousal,
            confidence,
        }
    }
}

/// 바운딩 박스
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl BoundingBox {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn area(&self) -> f32 {
        self.width * self.height
    }
}

/// 감정별 신뢰도 점수
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmotionScores {
    pub anger: f32,
    pub contempt: f32,
    pub disgust: f32,
    pub fear: f32,
    pub happiness: f32,
    pub neutral: f32,
    pub sadness: f32,
    pub surprise: f32,
}

impl EmotionScores {
    /// 가장 높은 점수의 감정 반환
    pub fn dominant_emotion(&self) -> EmotionType {
        let scores = [
            (self.anger, EmotionType::Angry),
            (self.contempt, EmotionType::Contempt),
            (self.disgust, EmotionType::Disgusted),
            (self.fear, EmotionType::Fearful),
            (self.happiness, EmotionType::Happy),
            (self.neutral, EmotionType::Neutral),
            (self.sadness, EmotionType::Sad),
            (self.surprise, EmotionType::Surprised),
        ];

        scores
            .into_iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, e)| e)
            .unwrap_or(EmotionType::Neutral)
    }

    /// 최대 점수
    pub fn max_score(&self) -> f32 {
        [
            self.anger, self.contempt, self.disgust, self.fear,
            self.happiness, self.neutral, self.sadness, self.surprise,
        ]
        .into_iter()
        .fold(0.0f32, |a, b| a.max(b))
    }

    /// valence-arousal 변환
    /// Russell's circumplex model 기반
    pub fn to_valence_arousal(&self) -> (f32, f32) {
        // 감정별 valence, arousal 매핑
        let mappings = [
            (self.happiness, 0.8, 0.6),   // 행복: 긍정, 중상 각성
            (self.surprise, 0.2, 0.8),    // 놀람: 약긍정, 고각성
            (self.anger, -0.7, 0.8),      // 분노: 부정, 고각성
            (self.fear, -0.6, 0.7),       // 공포: 부정, 고각성
            (self.disgust, -0.6, 0.4),    // 혐오: 부정, 중각성
            (self.sadness, -0.7, 0.3),    // 슬픔: 부정, 저각성
            (self.contempt, -0.4, 0.3),   // 경멸: 약부정, 저각성
            (self.neutral, 0.0, 0.3),     // 중립: 중간, 저각성
        ];

        let total_weight: f32 = mappings.iter().map(|(w, _, _)| w).sum();
        
        if total_weight < 0.001 {
            return (0.0, 0.5);
        }

        let valence: f32 = mappings.iter()
            .map(|(w, v, _)| w * v)
            .sum::<f32>() / total_weight;

        let arousal: f32 = mappings.iter()
            .map(|(w, _, a)| w * a)
            .sum::<f32>() / total_weight;

        (valence.clamp(-1.0, 1.0), arousal.clamp(0.0, 1.0))
    }
}

/// 표정 분석 백엔드 트레이트
#[async_trait]
pub trait FaceAnalyzer: Send + Sync {
    /// 이미지 파일에서 표정 분석
    async fn analyze_file(
        &self,
        file_path: &str,
        session_id: Uuid,
    ) -> Result<FaceAnalysisResult, FaceAnalyzerError>;

    /// 이미지 바이트에서 표정 분석
    async fn analyze_bytes(
        &self,
        image_data: &[u8],
        content_type: &str,
        session_id: Uuid,
    ) -> Result<FaceAnalysisResult, FaceAnalyzerError>;

    /// Base64 이미지에서 표정 분석
    async fn analyze_base64(
        &self,
        base64_data: &str,
        session_id: Uuid,
    ) -> Result<FaceAnalysisResult, FaceAnalyzerError>;

    /// 백엔드 이름
    fn name(&self) -> &str;
}

/// 이미지 형식
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    Bmp,
    Webp,
}

impl ImageFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "png" => Some(Self::Png),
            "gif" => Some(Self::Gif),
            "bmp" => Some(Self::Bmp),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }

    pub fn content_type(&self) -> &str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::Bmp => "image/bmp",
            Self::Webp => "image/webp",
        }
    }
}
