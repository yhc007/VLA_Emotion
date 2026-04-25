//! Azure Face API 구현
//!
//! https://learn.microsoft.com/azure/cognitive-services/face/

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::Deserialize;
use tracing::{debug, info};
use uuid::Uuid;

use crate::types::EmotionType;
use super::face_analyzer::{
    BoundingBox, EmotionScores, FaceAnalysisResult, FaceAnalyzer,
    FaceAnalyzerError, FaceEmotion,
};

/// Azure Face API 클라이언트
pub struct AzureFaceApi {
    endpoint: String,
    api_key: String,
    client: reqwest::Client,
}

impl AzureFaceApi {
    /// 새 클라이언트 생성
    pub fn new(endpoint: &str, api_key: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// 환경변수에서 설정 로드
    pub fn from_env() -> Result<Self, FaceAnalyzerError> {
        let endpoint = std::env::var("AZURE_FACE_ENDPOINT")
            .map_err(|_| FaceAnalyzerError::ConfigError("AZURE_FACE_ENDPOINT not set".to_string()))?;
        let api_key = std::env::var("AZURE_FACE_KEY")
            .map_err(|_| FaceAnalyzerError::ConfigError("AZURE_FACE_KEY not set".to_string()))?;
        Ok(Self::new(&endpoint, &api_key))
    }

    /// API 호출 (바이트)
    async fn call_api(&self, image_data: &[u8], content_type: &str) -> Result<Vec<AzureFaceResponse>, FaceAnalyzerError> {
        let url = format!(
            "{}/face/v1.0/detect?returnFaceId=false&returnFaceAttributes=emotion",
            self.endpoint
        );

        debug!(url = %url, size = image_data.len(), "Azure Face API 호출");

        let response = self.client
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .header("Content-Type", content_type)
            .body(image_data.to_vec())
            .send()
            .await
            .map_err(|e| FaceAnalyzerError::ApiError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(FaceAnalyzerError::ApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        response
            .json::<Vec<AzureFaceResponse>>()
            .await
            .map_err(|e| FaceAnalyzerError::ApiError(e.to_string()))
    }

    /// 응답을 FaceAnalysisResult로 변환
    fn to_result(&self, faces: Vec<AzureFaceResponse>, session_id: Uuid) -> FaceAnalysisResult {
        if faces.is_empty() {
            return FaceAnalysisResult::empty(session_id);
        }

        let face_emotions: Vec<FaceEmotion> = faces
            .into_iter()
            .enumerate()
            .map(|(idx, f)| {
                let scores = EmotionScores {
                    anger: f.face_attributes.emotion.anger,
                    contempt: f.face_attributes.emotion.contempt,
                    disgust: f.face_attributes.emotion.disgust,
                    fear: f.face_attributes.emotion.fear,
                    happiness: f.face_attributes.emotion.happiness,
                    neutral: f.face_attributes.emotion.neutral,
                    sadness: f.face_attributes.emotion.sadness,
                    surprise: f.face_attributes.emotion.surprise,
                };

                let emotion = scores.dominant_emotion();
                let bbox = BoundingBox::new(
                    f.face_rectangle.left as f32,
                    f.face_rectangle.top as f32,
                    f.face_rectangle.width as f32,
                    f.face_rectangle.height as f32,
                );

                FaceEmotion::new(&format!("face_{}", idx), bbox, emotion, scores)
            })
            .collect();

        // 가장 큰 얼굴을 primary로
        let primary = face_emotions
            .iter()
            .max_by(|a, b| {
                a.bounding_box.area()
                    .partial_cmp(&b.bounding_box.area())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned();

        FaceAnalysisResult {
            face_count: face_emotions.len(),
            primary_emotion: primary.as_ref().map(|f| f.emotion.clone()),
            primary_valence: primary.as_ref().map(|f| f.valence).unwrap_or(0.0),
            primary_arousal: primary.as_ref().map(|f| f.arousal).unwrap_or(0.5),
            faces: face_emotions,
            timestamp: chrono::Utc::now(),
            session_id,
        }
    }
}

#[async_trait]
impl FaceAnalyzer for AzureFaceApi {
    async fn analyze_file(
        &self,
        file_path: &str,
        session_id: Uuid,
    ) -> Result<FaceAnalysisResult, FaceAnalyzerError> {
        info!(file = %file_path, "파일 표정 분석 시작");

        let image_data = tokio::fs::read(file_path)
            .await
            .map_err(|e| FaceAnalyzerError::FileError(e.to_string()))?;

        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpeg");

        let content_type = match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            _ => "application/octet-stream",
        };

        let faces = self.call_api(&image_data, content_type).await?;
        info!(face_count = faces.len(), "표정 분석 완료");

        Ok(self.to_result(faces, session_id))
    }

    async fn analyze_bytes(
        &self,
        image_data: &[u8],
        content_type: &str,
        session_id: Uuid,
    ) -> Result<FaceAnalysisResult, FaceAnalyzerError> {
        info!(size = image_data.len(), "바이트 표정 분석 시작");

        let faces = self.call_api(image_data, content_type).await?;
        info!(face_count = faces.len(), "표정 분석 완료");

        Ok(self.to_result(faces, session_id))
    }

    async fn analyze_base64(
        &self,
        base64_data: &str,
        session_id: Uuid,
    ) -> Result<FaceAnalysisResult, FaceAnalyzerError> {
        // data:image/jpeg;base64, 접두사 제거
        let data = if base64_data.contains(',') {
            base64_data.split(',').nth(1).unwrap_or(base64_data)
        } else {
            base64_data
        };

        let image_data = BASE64
            .decode(data)
            .map_err(|e| FaceAnalyzerError::ImageFormatError(format!("Base64 디코딩 실패: {}", e)))?;

        // MIME 타입 추출 (있으면)
        let content_type = if base64_data.starts_with("data:") {
            base64_data
                .split(';')
                .next()
                .and_then(|s| s.strip_prefix("data:"))
                .unwrap_or("image/jpeg")
        } else {
            "image/jpeg"
        };

        self.analyze_bytes(&image_data, content_type, session_id).await
    }

    fn name(&self) -> &str {
        "Azure Face API"
    }
}

/// Azure Face API 응답
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzureFaceResponse {
    face_rectangle: AzureFaceRectangle,
    face_attributes: AzureFaceAttributes,
}

#[derive(Debug, Deserialize)]
struct AzureFaceRectangle {
    top: i32,
    left: i32,
    width: i32,
    height: i32,
}

#[derive(Debug, Deserialize)]
struct AzureFaceAttributes {
    emotion: AzureEmotion,
}

#[derive(Debug, Deserialize)]
struct AzureEmotion {
    anger: f32,
    contempt: f32,
    disgust: f32,
    fear: f32,
    happiness: f32,
    neutral: f32,
    sadness: f32,
    surprise: f32,
}

// ─── 간단한 Mock 구현 (테스트/개발용) ───

/// Mock Face Analyzer (개발/테스트용)
#[allow(dead_code)]
pub struct MockFaceAnalyzer;

#[allow(dead_code)]
impl MockFaceAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
#[allow(dead_code)]
impl FaceAnalyzer for MockFaceAnalyzer {
    async fn analyze_file(
        &self,
        _file_path: &str,
        session_id: Uuid,
    ) -> Result<FaceAnalysisResult, FaceAnalyzerError> {
        Ok(self.mock_result(session_id))
    }

    async fn analyze_bytes(
        &self,
        _image_data: &[u8],
        _content_type: &str,
        session_id: Uuid,
    ) -> Result<FaceAnalysisResult, FaceAnalyzerError> {
        Ok(self.mock_result(session_id))
    }

    async fn analyze_base64(
        &self,
        _base64_data: &str,
        session_id: Uuid,
    ) -> Result<FaceAnalysisResult, FaceAnalyzerError> {
        Ok(self.mock_result(session_id))
    }

    fn name(&self) -> &str {
        "Mock Face Analyzer"
    }
}

#[allow(dead_code)]
impl MockFaceAnalyzer {
    fn mock_result(&self, session_id: Uuid) -> FaceAnalysisResult {
        // 랜덤하게 감정 생성 (테스트용)
        let scores = EmotionScores {
            happiness: 0.1,
            sadness: 0.5,
            neutral: 0.3,
            anger: 0.05,
            fear: 0.03,
            surprise: 0.02,
            ..Default::default()
        };

        let emotion = scores.dominant_emotion();
        let bbox = BoundingBox::new(100.0, 100.0, 200.0, 200.0);
        let face = FaceEmotion::new("mock_face_0", bbox, emotion, scores);

        FaceAnalysisResult::single(face, session_id)
    }
}

#[allow(dead_code)]
impl Default for MockFaceAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
