//! LLM 연동 모듈
//!
//! Claude를 활용한 심리 분석 강화
//! - 심리 상태 종합 리포트 생성
//! - SPO 추출 보조
//! - 상담사 추천 멘트 제안

mod psych_reporter;
mod spo_assistant;

pub use psych_reporter::{PsychReporter, PsychReport, CounselorSuggestion};
pub use spo_assistant::{SpoAssistant, ValidationResult};
