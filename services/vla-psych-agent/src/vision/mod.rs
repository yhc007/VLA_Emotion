//! 비전 분석 모듈
//!
//! 영상/이미지에서 표정 인식
//! - Azure Face API
//! - MediaPipe (향후 로컬)

mod face_analyzer;
mod azure_face;

pub use face_analyzer::{FaceAnalyzer, FaceAnalysisResult, FaceEmotion, BoundingBox};
pub use azure_face::AzureFaceApi;
