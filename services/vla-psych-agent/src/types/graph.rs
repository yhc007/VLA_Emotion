use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::emotion::EmotionResult;
use super::spo::EntityType;

// ─── 그래프 노드 ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: NodeType,
    pub label: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub emotion_annotations: Vec<EmotionAnnotation>,
    pub created_at: DateTime<Utc>,
    pub session_id: Uuid,
}

impl GraphNode {
    pub fn from_entity(entity_id: &str, label: &str, entity_type: &EntityType, session_id: Uuid) -> Self {
        let node_type = match entity_type {
            EntityType::Person => NodeType::Person,
            EntityType::Organization => NodeType::Organization,
            EntityType::Concept => NodeType::Concept,
            EntityType::Event => NodeType::Event,
            EntityType::Location => NodeType::Location,
            _ => NodeType::Unknown,
        };
        Self {
            id: entity_id.to_string(),
            node_type,
            label: label.to_string(),
            properties: HashMap::new(),
            emotion_annotations: Vec::new(),
            created_at: Utc::now(),
            session_id,
        }
    }

    pub fn utterance(text: &str, timestamp: DateTime<Utc>, session_id: Uuid) -> Self {
        Self {
            id: format!("utt_{}", Uuid::new_v4()),
            node_type: NodeType::Utterance,
            label: text.to_string(),
            properties: HashMap::new(),
            emotion_annotations: Vec::new(),
            created_at: timestamp,
            session_id,
        }
    }

    pub fn emotion_state(emotion: &EmotionResult) -> Self {
        let mut properties = HashMap::new();
        properties.insert("valence".into(), serde_json::json!(emotion.valence));
        properties.insert("arousal".into(), serde_json::json!(emotion.arousal));
        properties.insert("emotion".into(), serde_json::json!(format!("{:?}", emotion.emotion)));

        Self {
            id: format!("emo_{}", emotion.id),
            node_type: NodeType::EmotionState,
            label: format!("{:?}", emotion.emotion),
            properties,
            emotion_annotations: Vec::new(),
            created_at: emotion.timestamp,
            session_id: emotion.session_id,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum NodeType {
    Person,
    Organization,
    Concept,
    Event,
    Location,
    Utterance,
    EmotionState,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmotionAnnotation {
    pub emotion_id: Uuid,
    pub valence: f32,
    pub arousal: f32,
    pub timestamp: DateTime<Utc>,
}

// ─── 그래프 엣지 ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: EdgeType,
    pub relation: String,
    pub weight: f32,
    pub timestamp: DateTime<Utc>,
    pub source_utterance: Option<String>,
}

impl GraphEdge {
    pub fn spo_relation(source: &str, target: &str, relation: &str, utterance: &str) -> Self {
        Self {
            id: format!("edge_{}", Uuid::new_v4()),
            source: source.to_string(),
            target: target.to_string(),
            edge_type: EdgeType::SpoRelation,
            relation: relation.to_string(),
            weight: 1.0,
            timestamp: Utc::now(),
            source_utterance: Some(utterance.to_string()),
        }
    }

    pub fn temporal_next(source: &str, target: &str) -> Self {
        Self {
            id: format!("edge_{}", Uuid::new_v4()),
            source: source.to_string(),
            target: target.to_string(),
            edge_type: EdgeType::TemporalNext,
            relation: "NEXT".to_string(),
            weight: 1.0,
            timestamp: Utc::now(),
            source_utterance: None,
        }
    }

    pub fn has_emotion(utterance_id: &str, emotion_id: &str) -> Self {
        Self {
            id: format!("edge_{}", Uuid::new_v4()),
            source: utterance_id.to_string(),
            target: emotion_id.to_string(),
            edge_type: EdgeType::HasEmotion,
            relation: "HAS_EMOTION".to_string(),
            weight: 1.0,
            timestamp: Utc::now(),
            source_utterance: None,
        }
    }

    pub fn contradicts(source: &str, target: &str) -> Self {
        Self {
            id: format!("edge_{}", Uuid::new_v4()),
            source: source.to_string(),
            target: target.to_string(),
            edge_type: EdgeType::Contradicts,
            relation: "CONTRADICTS".to_string(),
            weight: 1.0,
            timestamp: Utc::now(),
            source_utterance: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EdgeType {
    SpoRelation,
    TemporalNext,
    HasEmotion,
    RefersTo,
    AssociatedWith,
    Contradicts,
}

// ─── 그래프 업데이트 이벤트 ───

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphUpdated {
    pub added_nodes: Vec<GraphNode>,
    pub added_edges: Vec<GraphEdge>,
    pub version: u64,
    pub timestamp: DateTime<Utc>,
    pub session_id: Uuid,
}
