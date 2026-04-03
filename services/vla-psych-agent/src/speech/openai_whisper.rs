//! OpenAI Whisper API 구현
//!
//! https://platform.openai.com/docs/guides/speech-to-text

use async_trait::async_trait;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use tracing::{debug, info};
use uuid::Uuid;

use super::transcriber::{
    AudioFormat, SpeechTranscriber, TranscriberError, TranscriptResult, TranscriptSegment,
};

/// OpenAI Whisper API 클라이언트
pub struct OpenAIWhisper {
    api_key: String,
    model: String,
    language: Option<String>,
    client: reqwest::Client,
}

impl OpenAIWhisper {
    const API_URL: &'static str = "https://api.openai.com/v1/audio/transcriptions";

    /// 새 클라이언트 생성
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: "whisper-1".to_string(),
            language: Some("ko".to_string()), // 기본 한국어
            client: reqwest::Client::new(),
        }
    }

    /// 환경변수에서 API 키 로드
    pub fn from_env() -> Result<Self, TranscriberError> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| TranscriberError::ConfigError("OPENAI_API_KEY not set".to_string()))?;
        Ok(Self::new(&api_key))
    }

    /// 언어 설정
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = Some(language.to_string());
        self
    }

    /// 언어 자동 감지
    pub fn auto_detect_language(mut self) -> Self {
        self.language = None;
        self
    }

    /// API 호출
    async fn call_api(
        &self,
        audio_data: Vec<u8>,
        filename: &str,
        content_type: &str,
    ) -> Result<WhisperResponse, TranscriberError> {
        let file_part = Part::bytes(audio_data)
            .file_name(filename.to_string())
            .mime_str(content_type)
            .map_err(|e| TranscriberError::AudioFormatError(e.to_string()))?;

        let mut form = Form::new()
            .part("file", file_part)
            .text("model", self.model.clone())
            .text("response_format", "verbose_json")
            .text("timestamp_granularities[]", "segment");

        if let Some(ref lang) = self.language {
            form = form.text("language", lang.clone());
        }

        debug!(model = %self.model, "Whisper API 호출");

        let response = self
            .client
            .post(Self::API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| TranscriberError::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(TranscriberError::ApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        response
            .json::<WhisperResponse>()
            .await
            .map_err(|e| TranscriberError::ApiError(e.to_string()))
    }
}

#[async_trait]
impl SpeechTranscriber for OpenAIWhisper {
    async fn transcribe_file(
        &self,
        file_path: &str,
        session_id: Uuid,
    ) -> Result<TranscriptResult, TranscriberError> {
        info!(file = %file_path, "파일 음성 인식 시작");

        // 파일 읽기
        let audio_data = tokio::fs::read(file_path)
            .await
            .map_err(|e| TranscriberError::FileError(e.to_string()))?;

        // 확장자에서 형식 추출
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("wav");

        let format = AudioFormat::from_extension(ext)
            .ok_or_else(|| TranscriberError::AudioFormatError(format!("지원하지 않는 형식: {}", ext)))?;

        let filename = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav");

        let response = self.call_api(audio_data, filename, format.mime_type()).await?;
        
        Ok(response.into_result(session_id))
    }

    async fn transcribe_bytes(
        &self,
        audio_data: &[u8],
        format: AudioFormat,
        session_id: Uuid,
    ) -> Result<TranscriptResult, TranscriberError> {
        info!(size = audio_data.len(), "바이트 음성 인식 시작");

        let filename = match format {
            AudioFormat::Wav => "audio.wav",
            AudioFormat::Mp3 => "audio.mp3",
            AudioFormat::Webm => "audio.webm",
            AudioFormat::Ogg => "audio.ogg",
            AudioFormat::Flac => "audio.flac",
            AudioFormat::M4a => "audio.m4a",
        };

        let response = self
            .call_api(audio_data.to_vec(), filename, format.mime_type())
            .await?;
        
        Ok(response.into_result(session_id))
    }

    fn name(&self) -> &str {
        "OpenAI Whisper"
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec![
            "ko", "en", "ja", "zh", "es", "fr", "de", "it", "pt", "ru",
            "ar", "hi", "th", "vi", "id", "ms", "tl", "nl", "pl", "tr",
        ]
    }
}

/// Whisper API 응답
#[derive(Debug, Deserialize)]
struct WhisperResponse {
    text: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    duration: Option<f32>,
    #[serde(default)]
    segments: Option<Vec<WhisperSegment>>,
}

#[derive(Debug, Deserialize)]
struct WhisperSegment {
    id: usize,
    start: f32,
    end: f32,
    text: String,
    #[serde(default)]
    avg_logprob: Option<f32>,
}

impl WhisperResponse {
    fn into_result(self, session_id: Uuid) -> TranscriptResult {
        let segments: Vec<TranscriptSegment> = self
            .segments
            .unwrap_or_default()
            .into_iter()
            .map(|s| {
                let confidence = s.avg_logprob
                    .map(|lp| (lp.exp()).min(1.0).max(0.0))
                    .unwrap_or(0.9);
                TranscriptSegment::new(s.id, s.start, s.end, &s.text)
                    .with_confidence(confidence)
            })
            .collect();

        TranscriptResult::new(
            &self.text,
            self.language.as_deref().unwrap_or("unknown"),
            self.duration.unwrap_or(0.0),
            session_id,
        )
        .with_segments(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format() {
        assert_eq!(AudioFormat::from_extension("wav"), Some(AudioFormat::Wav));
        assert_eq!(AudioFormat::from_extension("MP3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_extension("xyz"), None);
    }

    #[test]
    fn test_mime_type() {
        assert_eq!(AudioFormat::Wav.mime_type(), "audio/wav");
        assert_eq!(AudioFormat::Mp3.mime_type(), "audio/mpeg");
    }
}
