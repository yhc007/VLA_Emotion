//! 심리 분석 리포트 생성기
//!
//! Claude를 활용해 심리 상태를 종합 분석하고 리포트 생성

use pekko_agent_llm::{ClaudeClient, LlmConfig, LlmRequest, ClaudeMessage, ContentBlock};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

use crate::types::{PsychState, SPOTriple, AnomalyAlert};

#[derive(Debug, Error)]
pub enum ReporterError {
    #[error("LLM 호출 실패: {0}")]
    LlmError(String),
    #[error("설정 오류: {0}")]
    ConfigError(String),
    #[error("파싱 오류: {0}")]
    ParseError(String),
}

/// 심리 분석 리포트 생성기
pub struct PsychReporter {
    client: ClaudeClient,
}

impl PsychReporter {
    /// 환경변수에서 설정 로드
    pub fn from_env() -> Result<Self, ReporterError> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| ReporterError::ConfigError("ANTHROPIC_API_KEY not set".to_string()))?;

        let config = LlmConfig {
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
            base_url: "https://api.anthropic.com".to_string(),
            max_tokens: 2048,
            temperature: 0.7,
            timeout_secs: 60,
            max_retries: 3,
            rate_limit_rpm: 100,
            token_budget_daily: 100_000,
        };

        Ok(Self {
            client: ClaudeClient::new(config),
        })
    }

    /// 심리 상태 종합 리포트 생성
    pub async fn generate_report(
        &self,
        psych_state: &PsychState,
        triples: &[SPOTriple],
        alerts: &[AnomalyAlert],
        conversation_summary: Option<&str>,
    ) -> Result<PsychReport, ReporterError> {
        let prompt = self.build_report_prompt(psych_state, triples, alerts, conversation_summary);

        info!(psych_state = ?psych_state.overall_state, "심리 리포트 생성 시작");

        let request = LlmRequest {
            system_prompt: SYSTEM_PROMPT.to_string(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: vec![ContentBlock::Text { text: prompt }],
            }],
            max_tokens: 2048,
            temperature: Some(0.7),
            tools: vec![],
            cacheable: false,
        };

        let response = self.client.send_message(&request).await
            .map_err(|e| ReporterError::LlmError(e.to_string()))?;

        // 응답 텍스트 추출
        let text = response.content.iter()
            .filter_map(|c| match c {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        // JSON 파싱 시도, 실패 시 텍스트 리포트로 반환
        if let Ok(report) = serde_json::from_str::<PsychReport>(&text) {
            Ok(report)
        } else {
            Ok(PsychReport {
                summary: text.clone(),
                key_findings: vec![],
                emotional_pattern: format!("{:?}", psych_state.overall_state),
                risk_level: self.calculate_risk_level(psych_state, alerts),
                recommendations: vec![],
                counselor_notes: text,
            })
        }
    }

    /// 상담사 추천 멘트 생성
    pub async fn suggest_response(
        &self,
        psych_state: &PsychState,
        last_utterance: &str,
        context: Option<&str>,
    ) -> Result<CounselorSuggestion, ReporterError> {
        let prompt = format!(
            r#"내담자의 마지막 발화와 심리 상태를 바탕으로 상담사가 할 수 있는 적절한 응답을 제안해주세요.

## 마지막 발화
"{}"

## 심리 상태
- 전반적 상태: {:?}
- 감정 추이: {:?}
- 말-표정 일치도: {:.0}%
- 이상 징후: {}개

{}

## 응답 형식 (JSON)
{{
  "empathetic_response": "공감적 응답",
  "follow_up_question": "후속 질문",
  "caution_points": ["주의할 점"],
  "therapeutic_approach": "추천 접근법"
}}"#,
            last_utterance,
            psych_state.overall_state,
            psych_state.emotion_trend,
            psych_state.coherence_score * 100.0,
            psych_state.anomalies.len(),
            context.map(|c| format!("## 맥락\n{}", c)).unwrap_or_default(),
        );

        let request = LlmRequest {
            system_prompt: COUNSELOR_PROMPT.to_string(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: vec![ContentBlock::Text { text: prompt }],
            }],
            max_tokens: 1024,
            temperature: Some(0.8),
            tools: vec![],
            cacheable: false,
        };

        let response = self.client.send_message(&request).await
            .map_err(|e| ReporterError::LlmError(e.to_string()))?;

        let text = response.content.iter()
            .filter_map(|c| match c {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        serde_json::from_str(&text)
            .map_err(|e| ReporterError::ParseError(format!("JSON 파싱 실패: {} - 원문: {}", e, text)))
    }

    fn build_report_prompt(
        &self,
        psych_state: &PsychState,
        triples: &[SPOTriple],
        alerts: &[AnomalyAlert],
        conversation_summary: Option<&str>,
    ) -> String {
        let triples_text = triples.iter()
            .map(|t| format!("({}) —[{} / {:?}]→ ({})", 
                t.subject.name, t.predicate.verb, t.predicate.polarity, t.object.name))
            .collect::<Vec<_>>()
            .join("\n");

        let alerts_text = alerts.iter()
            .map(|a| format!("- {:?} ({:?}): {}", a.alert_type, a.severity, a.recommended_action))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"다음 심리 분석 데이터를 바탕으로 종합 리포트를 작성해주세요.

## 심리 상태
- 전반적 상태: {:?}
- 감정 추이: {:?}
- 말-표정 일치도: {:.0}%
- 분석 윈도우: {}초

## 발화 패턴 (SPO 트리플)
{}

## 이상 징후
{}

{}

## 응답 형식 (JSON)
{{
  "summary": "한 문장 요약",
  "key_findings": ["주요 발견 1", "주요 발견 2"],
  "emotional_pattern": "감정 패턴 설명",
  "risk_level": "low|medium|high|critical",
  "recommendations": ["권장 사항 1", "권장 사항 2"],
  "counselor_notes": "상담사 참고 노트"
}}"#,
            psych_state.overall_state,
            psych_state.emotion_trend,
            psych_state.coherence_score * 100.0,
            psych_state.analysis_window_secs,
            if triples_text.is_empty() { "(없음)" } else { &triples_text },
            if alerts_text.is_empty() { "(없음)" } else { &alerts_text },
            conversation_summary.map(|s| format!("## 대화 요약\n{}", s)).unwrap_or_default(),
        )
    }

    fn calculate_risk_level(&self, psych_state: &PsychState, alerts: &[AnomalyAlert]) -> String {
        use crate::types::Severity;

        let has_critical = alerts.iter().any(|a| a.severity == Severity::Critical);
        let has_high = alerts.iter().any(|a| a.severity == Severity::High);
        let coherence_low = psych_state.coherence_score < 0.5;

        if has_critical || (has_high && coherence_low) {
            "critical".to_string()
        } else if has_high || coherence_low {
            "high".to_string()
        } else if !alerts.is_empty() {
            "medium".to_string()
        } else {
            "low".to_string()
        }
    }
}

/// 심리 분석 리포트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsychReport {
    /// 한 문장 요약
    pub summary: String,
    /// 주요 발견
    pub key_findings: Vec<String>,
    /// 감정 패턴 설명
    pub emotional_pattern: String,
    /// 위험 수준
    pub risk_level: String,
    /// 권장 사항
    pub recommendations: Vec<String>,
    /// 상담사 참고 노트
    pub counselor_notes: String,
}

/// 상담사 추천 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounselorSuggestion {
    /// 공감적 응답
    pub empathetic_response: String,
    /// 후속 질문
    pub follow_up_question: String,
    /// 주의할 점
    pub caution_points: Vec<String>,
    /// 추천 접근법
    pub therapeutic_approach: String,
}

const SYSTEM_PROMPT: &str = r#"당신은 임상심리 전문가입니다. 
VLA(Vision-Language-Action) 시스템에서 수집한 발화 패턴과 감정 데이터를 분석하여 
객관적이고 전문적인 심리 분석 리포트를 작성합니다.

주의사항:
- 과잉 해석을 피하고 데이터에 기반한 분석만 수행
- 진단보다는 관찰과 권장 사항에 집중
- 위험 신호가 있을 경우 명확히 표시
- 항상 JSON 형식으로 응답"#;

const COUNSELOR_PROMPT: &str = r#"당신은 경험 많은 상담 심리사입니다.
내담자의 현재 심리 상태와 발화를 바탕으로 적절한 상담 응답을 제안합니다.

원칙:
- 공감과 수용을 기본으로
- 개방형 질문으로 탐색 유도
- 내담자의 감정을 먼저 인정
- 판단이나 조언보다 반영과 명료화 우선
- 항상 JSON 형식으로 응답"#;
