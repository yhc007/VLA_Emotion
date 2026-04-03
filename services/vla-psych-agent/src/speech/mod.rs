//! 음성 인식 모듈
//!
//! 음성 → 텍스트 변환 (Speech-to-Text)
//! - OpenAI Whisper API
//! - 로컬 whisper-rs (향후)

mod transcriber;
mod openai_whisper;

pub use transcriber::{SpeechTranscriber, TranscriptResult, TranscriptSegment, AudioFormat};
pub use openai_whisper::OpenAIWhisper;
