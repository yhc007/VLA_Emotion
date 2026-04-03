use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── 얼굴 프레임 ───

#[derive(Clone, Debug)]
pub struct FaceFrame {
    pub face_data: Vec<u8>,
    pub bounding_box: BoundingBox,
    pub timestamp: DateTime<Utc>,
    pub session_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

// ─── 감정 분석 결과 ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmotionResult {
    pub id: Uuid,
    pub emotion: EmotionType,
    pub valence: f32,  // -1.0 (부정) ~ 1.0 (긍정)
    pub arousal: f32,  // 0.0 (낮음) ~ 1.0 (높음)
    pub confidence: f32,
    pub timestamp: DateTime<Utc>,
    pub session_id: Uuid,
    pub speaker_id: String,
}

impl EmotionResult {
    pub fn new(
        emotion: EmotionType,
        valence: f32,
        arousal: f32,
        speaker_id: &str,
        session_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            emotion,
            valence: valence.clamp(-1.0, 1.0),
            arousal: arousal.clamp(0.0, 1.0),
            confidence: 0.0,
            timestamp: Utc::now(),
            session_id,
            speaker_id: speaker_id.to_string(),
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// 말-표정 불일치 여부 판단
    /// SPO의 polarity와 비교하여 불일치 감지
    ///
    /// Neutral 극성인 경우에도 표정의 valence가 강하게 부정/긍정이면
    /// 불일치로 판단한다. 예: "나는 괜찮아"(Neutral) + 슬픈 표정(valence=-0.5)
    pub fn contradicts_polarity(&self, polarity: &super::spo::Polarity) -> bool {
        match polarity {
            super::spo::Polarity::Positive => self.valence < -0.3,
            super::spo::Polarity::Negative => self.valence > 0.3,
            super::spo::Polarity::Neutral => self.valence.abs() > 0.4,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EmotionType {
    Happy,
    Sad,
    Angry,
    Surprised,
    Fearful,
    Disgusted,
    Neutral,
    Contempt,
}

impl EmotionType {
    /// 기본 valence 값 반환
    pub fn default_valence(&self) -> f32 {
        match self {
            EmotionType::Happy => 0.8,
            EmotionType::Sad => -0.6,
            EmotionType::Angry => -0.7,
            EmotionType::Surprised => 0.1,
            EmotionType::Fearful => -0.5,
            EmotionType::Disgusted => -0.6,
            EmotionType::Neutral => 0.0,
            EmotionType::Contempt => -0.4,
        }
    }

    /// 기본 arousal 값 반환
    pub fn default_arousal(&self) -> f32 {
        match self {
            EmotionType::Happy => 0.6,
            EmotionType::Sad => 0.3,
            EmotionType::Angry => 0.8,
            EmotionType::Surprised => 0.9,
            EmotionType::Fearful => 0.7,
            EmotionType::Disgusted => 0.5,
            EmotionType::Neutral => 0.2,
            EmotionType::Contempt => 0.4,
        }
    }
}
