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
