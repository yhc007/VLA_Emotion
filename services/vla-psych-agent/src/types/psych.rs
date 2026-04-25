use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── 심리 상태 ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PsychState {
    pub session_id: Uuid,
    pub overall_state: PsychStateType,
    pub emotion_trend: EmotionTrend,
    pub coherence_score: f32, // 0.0 ~ 1.0 (말-표정 일치도)
    pub anomalies: Vec<Anomaly>,
    pub timestamp: DateTime<Utc>,
    pub analysis_window_secs: u64,
}

impl PsychState {
    pub fn new(session_id: Uuid) -> Self {
        Self {
            session_id,
            overall_state: PsychStateType::Normal,
            emotion_trend: EmotionTrend::Stable,
            coherence_score: 1.0,
            anomalies: Vec::new(),
            timestamp: Utc::now(),
            analysis_window_secs: 60,
        }
    }

    pub fn is_concerning(&self) -> bool {
        self.coherence_score < 0.5
            || !self.anomalies.is_empty()
            || matches!(
                self.overall_state,
                PsychStateType::Distressed | PsychStateType::Agitated
            )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PsychStateType {
    Normal,
    Stressed,
    Distressed,
    Agitated,
    Withdrawn,
    Evasive,     // 회피적 (말-표정 불일치 지속)
    Elevated,    // 고양 상태
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum EmotionTrend {
    Stable,
    Improving,
    Declining,
    Volatile,    // 급변
}

// ─── 이상 탐지 ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Anomaly {
    pub anomaly_type: AnomalyType,
    pub severity: Severity,
    pub evidence: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub enum AnomalyType {
    EmotionSuddenChange,       // 감정 급변
    SpeechEmotionMismatch,     // 말-표정 불일치
    TopicRepetition,           // 특정 주제 반복
    SelfReferencePattern,      // 자기 참조 패턴 변화
    NegativeEscalation,        // 부정적 감정 에스컬레이션
    ResponseAvoidance,         // 특정 주제 회피
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

// ─── 이상 알림 ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnomalyAlert {
    pub id: Uuid,
    pub alert_type: AnomalyType,
    pub severity: Severity,
    pub evidence: Vec<String>,
    pub session_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub recommended_action: String,
}

impl AnomalyAlert {
    pub fn from_anomaly(anomaly: &Anomaly, session_id: Uuid) -> Self {
        let recommended_action = match anomaly.anomaly_type {
            AnomalyType::EmotionSuddenChange => "감정 변화 원인 탐색 필요",
            AnomalyType::SpeechEmotionMismatch => "발화 내용과 표정 간 불일치 확인",
            AnomalyType::TopicRepetition => "반복 주제에 대한 심층 탐색 필요",
            AnomalyType::SelfReferencePattern => "자기 인식 변화 모니터링",
            AnomalyType::NegativeEscalation => "부정적 감정 심화 — 개입 고려",
            AnomalyType::ResponseAvoidance => "회피 주제에 대한 점진적 접근",
        };

        Self {
            id: Uuid::new_v4(),
            alert_type: anomaly.anomaly_type.clone(),
            severity: anomaly.severity.clone(),
            evidence: anomaly.evidence.clone(),
            session_id,
            timestamp: anomaly.timestamp,
            recommended_action: recommended_action.to_string(),
        }
    }
}
