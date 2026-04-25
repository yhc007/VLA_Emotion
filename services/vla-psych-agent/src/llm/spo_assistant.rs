//! SPO 추출 보조
//!
//! Claude를 활용해 복잡한 문장의 SPO 추출 보조

use pekko_agent_llm::{ClaudeClient, LlmConfig, LlmRequest, ClaudeMessage, ContentBlock};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;
use uuid::Uuid;

use crate::types::{Entity, EntityType, Predicate, Polarity, SPOTriple};

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum SpoAssistantError {
    #[error("LLM 호출 실패: {0}")]
    LlmError(String),
    #[error("설정 오류: {0}")]
    ConfigError(String),
    #[error("파싱 오류: {0}")]
    ParseError(String),
}

/// SPO 추출 보조
#[allow(dead_code)]
pub struct SpoAssistant {
    client: ClaudeClient,
}

#[allow(dead_code)]
impl SpoAssistant {
    /// 환경변수에서 설정 로드
    pub fn from_env() -> Result<Self, SpoAssistantError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| SpoAssistantError::ConfigError("ANTHROPIC_API_KEY not set".to_string()))?;

        let config = LlmConfig {
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            max_tokens: 1024,
            temperature: 0.3, // 더 결정적으로
            timeout_secs: 30,
            max_retries: 2,
            rate_limit_rpm: 100,
            token_budget_daily: 100_000,
        };

        Ok(Self {
            client: ClaudeClient::new(config),
        })
    }

    /// 복잡한 문장에서 SPO 추출
    pub async fn extract_spo(
        &self,
        text: &str,
        speaker_id: &str,
        session_id: Uuid,
    ) -> Result<Vec<SPOTriple>, SpoAssistantError> {
        let prompt = format!(
            r#"다음 한국어 문장에서 SPO(주어-술어-목적어) 트리플을 추출해주세요.

문장: "{}"

## 규칙
1. 주어가 생략된 경우 "화자"로 표시
2. 목적어가 없는 경우 "_implicit"로 표시
3. 술어의 극성(polarity) 판단: positive/negative/neutral
4. 강도(intensity): 0.0 ~ 1.0

## 응답 형식 (JSON 배열)
[
  {{
    "subject": {{ "name": "이름", "type": "Person|Organization|Concept|Event|Location" }},
    "predicate": {{ "verb": "동사/형용사", "polarity": "positive|negative|neutral", "intensity": 0.8 }},
    "object": {{ "name": "이름", "type": "Person|Organization|Concept|Event|Location" }}
  }}
]"#,
            text
        );

        info!(text = %text, "LLM SPO 추출 시작");

        let request = LlmRequest {
            system_prompt: SPO_SYSTEM_PROMPT.to_string(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: vec![ContentBlock::Text { text: prompt }],
            }],
            max_tokens: 1024,
            temperature: Some(0.3),
            tools: vec![],
            cacheable: false,
        };

        let response = self.client.send_message(&request).await
            .map_err(|e| SpoAssistantError::LlmError(e.to_string()))?;

        let text_response = response.content.iter()
            .filter_map(|c| match c {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        // JSON 파싱
        let spo_results: Vec<SpoExtractResult> = serde_json::from_str(&text_response)
            .map_err(|e| SpoAssistantError::ParseError(format!("JSON 파싱 실패: {} - 원문: {}", e, text_response)))?;

        // SPOTriple로 변환
        let triples: Vec<SPOTriple> = spo_results.into_iter()
            .map(|r| {
                let subject = Entity {
                    id: format!("{}_{}", r.subject.entity_type.to_lowercase(), r.subject.name),
                    name: r.subject.name,
                    entity_type: parse_entity_type(&r.subject.entity_type),
                    attributes: std::collections::HashMap::new(),
                };

                let object = Entity {
                    id: format!("{}_{}", r.object.entity_type.to_lowercase(), r.object.name),
                    name: r.object.name,
                    entity_type: parse_entity_type(&r.object.entity_type),
                    attributes: std::collections::HashMap::new(),
                };

                let polarity = match r.predicate.polarity.to_lowercase().as_str() {
                    "positive" => Polarity::Positive,
                    "negative" => Polarity::Negative,
                    _ => Polarity::Neutral,
                };

                let predicate = Predicate::new(&r.predicate.verb, polarity)
                    .with_intensity(r.predicate.intensity.unwrap_or(0.5));

                SPOTriple::new(
                    subject,
                    predicate,
                    object,
                    text,
                    speaker_id,
                    session_id,
                    0.9, // LLM 추출 신뢰도
                )
            })
            .collect();

        info!(count = triples.len(), "LLM SPO 추출 완료");

        Ok(triples)
    }

    /// 룰 기반 추출 결과 검증/보정
    pub async fn validate_spo(
        &self,
        text: &str,
        rule_based_triples: &[SPOTriple],
    ) -> Result<ValidationResult, SpoAssistantError> {
        let triples_json = serde_json::to_string_pretty(rule_based_triples)
            .unwrap_or_else(|_| "[]".to_string());

        let prompt = format!(
            r#"다음 한국어 문장과 룰 기반 SPO 추출 결과를 검토하고 보정해주세요.

## 원문
"{}"

## 룰 기반 추출 결과
{}

## 요청
1. 추출 결과가 올바른지 검증
2. 누락된 SPO가 있는지 확인
3. 잘못된 극성(polarity)이 있는지 확인

## 응답 형식 (JSON)
{{
  "is_valid": true/false,
  "issues": ["문제점 1", "문제점 2"],
  "missing_triples": [...],
  "corrections": [{{ "original": "...", "corrected": "..." }}]
}}"#,
            text, triples_json
        );

        let request = LlmRequest {
            system_prompt: SPO_SYSTEM_PROMPT.to_string(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: vec![ContentBlock::Text { text: prompt }],
            }],
            max_tokens: 1024,
            temperature: Some(0.3),
            tools: vec![],
            cacheable: false,
        };

        let response = self.client.send_message(&request).await
            .map_err(|e| SpoAssistantError::LlmError(e.to_string()))?;

        let text_response = response.content.iter()
            .filter_map(|c| match c {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        serde_json::from_str(&text_response)
            .map_err(|e| SpoAssistantError::ParseError(format!("JSON 파싱 실패: {}", e)))
    }
}

#[allow(dead_code)]
fn parse_entity_type(s: &str) -> EntityType {
    match s.to_lowercase().as_str() {
        "person" => EntityType::Person,
        "organization" => EntityType::Organization,
        "concept" => EntityType::Concept,
        "event" => EntityType::Event,
        "location" => EntityType::Location,
        _ => EntityType::Unknown,
    }
}

/// SPO 추출 결과 (JSON 파싱용)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SpoExtractResult {
    subject: EntityDto,
    predicate: PredicateDto,
    object: EntityDto,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct EntityDto {
    name: String,
    #[serde(rename = "type")]
    entity_type: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct PredicateDto {
    verb: String,
    polarity: String,
    intensity: Option<f32>,
}

/// 검증 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub issues: Vec<String>,
    pub missing_triples: Vec<serde_json::Value>,
    pub corrections: Vec<Correction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Correction {
    pub original: String,
    pub corrected: String,
}

#[allow(dead_code)]
const SPO_SYSTEM_PROMPT: &str = r#"당신은 한국어 자연어 처리 전문가입니다.
문장에서 SPO(주어-술어-목적어) 트리플을 정확하게 추출합니다.

한국어 특성:
- SOV 어순 (주어-목적어-술어)
- 주어 생략이 흔함
- 조사로 문법 관계 표시 (-이/가, -을/를, -에게 등)

추출 원칙:
- 암시적 주어는 "화자"로 표시
- 감정/상태 표현의 극성(polarity) 정확히 판단
- 강도(intensity)는 부사어와 어미로 판단

항상 JSON 형식으로 응답"#;
