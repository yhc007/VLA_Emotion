use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use crate::components::report_viewer::Report;

const API_BASE: &str = "/api/v1";

#[derive(Debug, Serialize)]
struct CreateSessionRequest {
    client_id: String,
    counselor_id: String,
}

#[derive(Debug, Deserialize)]
struct CreateSessionResponse {
    session_id: String,
}

#[derive(Debug, Deserialize)]
struct ReportResponse {
    report: Report,
}

/// 새 상담 세션 생성
pub async fn create_session() -> Result<String, String> {
    let req = CreateSessionRequest {
        client_id: "client-001".to_string(),
        counselor_id: "counselor-001".to_string(),
    };
    
    let response = Request::post(&format!("{}/session/create", API_BASE))
        .header("Content-Type", "application/json")
        .json(&req)
        .map_err(|e| format!("요청 생성 실패: {}", e))?
        .send()
        .await
        .map_err(|e| format!("요청 전송 실패: {}", e))?;
    
    if !response.ok() {
        return Err(format!("서버 에러: {}", response.status()));
    }
    
    let data: CreateSessionResponse = response
        .json()
        .await
        .map_err(|e| format!("응답 파싱 실패: {}", e))?;
    
    Ok(data.session_id)
}

/// 세션 종료
pub async fn end_session(session_id: &str) -> Result<(), String> {
    let response = Request::post(&format!("{}/session/{}/end", API_BASE, session_id))
        .send()
        .await
        .map_err(|e| format!("요청 전송 실패: {}", e))?;
    
    if !response.ok() {
        return Err(format!("서버 에러: {}", response.status()));
    }
    
    Ok(())
}

/// AI 리포트 생성
pub async fn generate_report(session_id: &str) -> Result<Report, String> {
    #[derive(Serialize)]
    struct ReportRequest {
        session_id: String,
    }
    
    let req = ReportRequest {
        session_id: session_id.to_string(),
    };
    
    let response = Request::post(&format!("{}/report/generate", API_BASE))
        .header("Content-Type", "application/json")
        .json(&req)
        .map_err(|e| format!("요청 생성 실패: {}", e))?
        .send()
        .await
        .map_err(|e| format!("요청 전송 실패: {}", e))?;
    
    if !response.ok() {
        return Err(format!("서버 에러: {}", response.status()));
    }
    
    let data: ReportResponse = response
        .json()
        .await
        .map_err(|e| format!("응답 파싱 실패: {}", e))?;
    
    Ok(data.report)
}

/// 표정 분석 요청
pub async fn analyze_emotion(session_id: &str, image_data: &str) -> Result<String, String> {
    #[derive(Serialize)]
    struct EmotionRequest {
        session_id: String,
        image: String,
    }
    
    let req = EmotionRequest {
        session_id: session_id.to_string(),
        image: image_data.to_string(),
    };
    
    let response = Request::post(&format!("{}/emotion/analyze", API_BASE))
        .header("Content-Type", "application/json")
        .json(&req)
        .map_err(|e| format!("요청 생성 실패: {}", e))?
        .send()
        .await
        .map_err(|e| format!("요청 전송 실패: {}", e))?;
    
    if !response.ok() {
        return Err(format!("서버 에러: {}", response.status()));
    }
    
    let text = response
        .text()
        .await
        .map_err(|e| format!("응답 읽기 실패: {}", e))?;
    
    Ok(text)
}

// ── Speaker Diarization API ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakerSegment {
    pub start_time: f64,
    pub end_time: f64,
    pub speaker_id: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiarizationResult {
    pub segments: Vec<SpeakerSegment>,
    pub num_speakers: usize,
    pub duration: f64,
    pub processing_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioChunk {
    pub timestamp: f64,
    pub duration: f64,
    pub sample_rate: i32,
    pub audio_data: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeAnalysisResult {
    pub timestamp: f64,
    pub estimated_speaker: String,
    pub features: Option<VoiceFeatures>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceFeatures {
    pub f0: Option<f64>,
    pub spectral_centroid: f64,
    pub mfcc: Vec<f64>,
}

// Note: Python Audio Service functions removed - using Rust-based speaker diarization instead

// ── Saved Sessions API ──

use crate::components::saved_sessions::{SavedSessionSummary, SavedSessionData};

/// 저장된 세션 목록 조회
pub async fn fetch_session_list() -> Result<Vec<SavedSessionSummary>, String> {
    let response = Request::get(&format!("{}/sessions", API_BASE))
        .send()
        .await
        .map_err(|e| format!("세션 목록 조회 실패: {}", e))?;

    if !response.ok() {
        return Err(format!("서버 에러: {}", response.status()));
    }

    let sessions: Vec<SavedSessionSummary> = response
        .json()
        .await
        .map_err(|e| format!("응답 파싱 실패: {}", e))?;

    Ok(sessions)
}

/// 특정 세션 데이터 로드
pub async fn fetch_session_data(session_id: &str) -> Result<SavedSessionData, String> {
    let response = Request::get(&format!("{}/sessions/{}/data", API_BASE, session_id))
        .send()
        .await
        .map_err(|e| format!("세션 데이터 로드 실패: {}", e))?;

    if !response.ok() {
        return Err(format!("서버 에러: {}", response.status()));
    }

    let data: SavedSessionData = response
        .json()
        .await
        .map_err(|e| format!("응답 파싱 실패: {}", e))?;

    Ok(data)
}
