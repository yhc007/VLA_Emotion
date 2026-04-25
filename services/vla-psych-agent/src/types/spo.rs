use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ─── SPO 트리플 (Subject-Predicate-Object) ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SPOTriple {
    pub id: Uuid,
    pub subject: Entity,
    pub predicate: Predicate,
    pub object: Entity,
    pub timestamp: DateTime<Utc>,
    pub confidence: f32,
    pub source_utterance: String,
    pub speaker_id: String,
    pub session_id: Uuid,
}

impl SPOTriple {
    pub fn new(
        subject: Entity,
        predicate: Predicate,
        object: Entity,
        source_utterance: &str,
        speaker_id: &str,
        session_id: Uuid,
        confidence: f32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            subject,
            predicate,
            object,
            timestamp: Utc::now(),
            confidence,
            source_utterance: source_utterance.to_string(),
            speaker_id: speaker_id.to_string(),
            session_id,
        }
    }
}

// ─── 엔티티 (주어/목적어) ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub entity_type: EntityType,
    pub attributes: HashMap<String, serde_json::Value>,
}

impl Entity {
    pub fn person(name: &str) -> Self {
        Self {
            id: format!("person_{}", name),
            name: name.to_string(),
            entity_type: EntityType::Person,
            attributes: HashMap::new(),
        }
    }

    pub fn organization(name: &str) -> Self {
        Self {
            id: format!("org_{}", name),
            name: name.to_string(),
            entity_type: EntityType::Organization,
            attributes: HashMap::new(),
        }
    }

    pub fn concept(name: &str) -> Self {
        Self {
            id: format!("concept_{}", name),
            name: name.to_string(),
            entity_type: EntityType::Concept,
            attributes: HashMap::new(),
        }
    }

    pub fn event(name: &str) -> Self {
        Self {
            id: format!("event_{}", name),
            name: name.to_string(),
            entity_type: EntityType::Event,
            attributes: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum EntityType {
    Person,
    Organization,
    Concept,
    Event,
    Location,
    Object,
    Unknown,
}

// ─── 술어 (Predicate) ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Predicate {
    pub id: String,
    pub verb: String,
    pub tense: Tense,
    pub polarity: Polarity,
    pub modality: Modality,
    pub intensity: f32, // 0.0 ~ 1.0
}

impl Predicate {
    pub fn new(verb: &str, polarity: Polarity) -> Self {
        Self {
            id: format!("pred_{}", verb),
            verb: verb.to_string(),
            tense: Tense::Present,
            polarity,
            modality: Modality::Declarative,
            intensity: 0.5,
        }
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    #[allow(dead_code)]
    pub fn with_tense(mut self, tense: Tense) -> Self {
        self.tense = tense;
        self
    }

    #[allow(dead_code)]
    pub fn with_modality(mut self, modality: Modality) -> Self {
        self.modality = modality;
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub enum Tense {
    Past,
    Present,
    Future,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Polarity {
    Positive,
    Negative,
    Neutral,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub enum Modality {
    Declarative,  // 평서: "~이다"
    Interrogative, // 의문: "~인가?"
    Imperative,   // 명령: "~해라"
    Subjunctive,  // 가정: "~라면"
    Desiderative, // 희망: "~하고 싶다"
}

// ─── 음성 세그먼트 ───

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct SpeechSegment {
    pub audio_bytes: Vec<u8>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub speaker_id: String,
}

// ─── STT 결과 ───

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptResult {
    pub text: String,
    pub confidence: f32,
    pub timestamp: DateTime<Utc>,
    pub speaker_id: String,
    pub session_id: Uuid,
    pub language: String,
}

#[allow(dead_code)]
impl TranscriptResult {
    pub fn new(text: &str, speaker_id: &str, session_id: Uuid, confidence: f32) -> Self {
        Self {
            text: text.to_string(),
            confidence,
            timestamp: Utc::now(),
            speaker_id: speaker_id.to_string(),
            session_id,
            language: "ko".to_string(),
        }
    }
}
