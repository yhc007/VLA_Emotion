//! API Handlers

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use tracing::{info, warn, error};
use uuid::Uuid;

use crate::types::EmotionResult;
use crate::speech::{OpenAIWhisper, SpeechTranscriber, AudioFormat};
use crate::vision::{AzureFaceApi, FaceAnalyzer, FaceAnalysisResult};
use super::dto::*;
use super::state::AppState;

/// GET /health
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "vla-psych-agent".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// POST /api/v1/session/start
pub async fn start_session(
    State(state): State<AppState>,
) -> Json<StartSessionResponse> {
    let session_id = state.create_session().await;
    
    info!(session_id = %session_id, "새 세션 시작");
    
    Json(StartSessionResponse {
        session_id,
        message: "세션이 시작되었습니다".to_string(),
    })
}

/// POST /api/v1/analyze/text
pub async fn analyze_text(
    State(state): State<AppState>,
    Json(req): Json<AnalyzeTextRequest>,
) -> Result<Json<AnalyzeTextResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 세션 확인
    let mut agent = match state.get_session(&req.session_id).await {
        Some(agent) => agent,
        None => {
            warn!(session_id = %req.session_id, "세션을 찾을 수 없음");
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "SESSION_NOT_FOUND".to_string(),
                    message: format!("세션 {}를 찾을 수 없습니다", req.session_id),
                }),
            ));
        }
    };

    // 감정 데이터 변환
    let emotion = req.emotion.map(|e| {
        EmotionResult::new(
            e.to_emotion_type(),
            e.valence,
            e.arousal,
            "api_input",
            req.session_id,
        )
    });

    // VLA 파이프라인 실행
    info!(
        session_id = %req.session_id,
        text = %req.text,
        has_emotion = emotion.is_some(),
        "텍스트 분석 시작"
    );

    let result = agent.process_text_input(&req.text, emotion);

    // 세션 업데이트
    state.update_session(&req.session_id, agent).await;

    let coherence_score = result.psych_state.coherence_score;
    
    Ok(Json(AnalyzeTextResponse {
        session_id: req.session_id,
        triples: result.triples,
        psych_state: result.psych_state,
        coherence_score,
        contradiction_detected: result.contradiction_detected,
        alerts: result.alerts,
        graph_summary: result.graph_summary,
    }))
}

/// GET /api/v1/session/:id/state
pub async fn get_session_state(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<SessionStateResponse>, (StatusCode, Json<ErrorResponse>)> {
    let agent = match state.get_session(&session_id).await {
        Some(agent) => agent,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "SESSION_NOT_FOUND".to_string(),
                    message: format!("세션 {}를 찾을 수 없습니다", session_id),
                }),
            ));
        }
    };

    let (psych_state, utterance_count, node_count, edge_count) = agent.get_session_stats();

    Ok(Json(SessionStateResponse {
        session_id,
        psych_state,
        utterance_count,
        graph_node_count: node_count,
        graph_edge_count: edge_count,
    }))
}

/// GET /api/v1/session/:id/graph
pub async fn get_session_graph(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<GraphResponse>, (StatusCode, Json<ErrorResponse>)> {
    let agent = match state.get_session(&session_id).await {
        Some(agent) => agent,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "SESSION_NOT_FOUND".to_string(),
                    message: format!("세션 {}를 찾을 수 없습니다", session_id),
                }),
            ));
        }
    };

    let (nodes, edges) = agent.get_graph_data();

    Ok(Json(GraphResponse {
        session_id,
        nodes: nodes.into_iter().map(|(id, label, node_type)| GraphNode {
            id,
            label,
            node_type,
        }).collect(),
        edges: edges.into_iter().map(|(source, target, label, weight)| GraphEdge {
            source,
            target,
            label,
            weight,
        }).collect(),
    }))
}

/// POST /api/v1/analyze/audio
/// 
/// 음성 파일 업로드 → Whisper 변환 → VLA 분석
pub async fn analyze_audio(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<AudioAnalyzeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut session_id: Option<Uuid> = None;
    let mut audio_data: Option<Vec<u8>> = None;
    let mut audio_format: Option<AudioFormat> = None;
    let mut emotion_input: Option<EmotionInput> = None;

    // multipart 필드 파싱
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "MULTIPART_ERROR".to_string(),
            message: e.to_string(),
        }))
    })? {
        let name = field.name().unwrap_or("").to_string();
        
        match name.as_str() {
            "session_id" => {
                let text = field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error: "FIELD_ERROR".to_string(),
                        message: format!("session_id 읽기 실패: {}", e),
                    }))
                })?;
                session_id = Some(Uuid::parse_str(&text).map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error: "INVALID_UUID".to_string(),
                        message: format!("잘못된 session_id: {}", e),
                    }))
                })?);
            }
            "audio" => {
                // 파일명에서 형식 추출
                if let Some(filename) = field.file_name() {
                    let ext = std::path::Path::new(filename)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("wav");
                    audio_format = AudioFormat::from_extension(ext);
                }
                
                audio_data = Some(field.bytes().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error: "AUDIO_READ_ERROR".to_string(),
                        message: format!("오디오 읽기 실패: {}", e),
                    }))
                })?.to_vec());
            }
            "emotion" => {
                let text = field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error: "FIELD_ERROR".to_string(),
                        message: format!("emotion 읽기 실패: {}", e),
                    }))
                })?;
                emotion_input = serde_json::from_str(&text).ok();
            }
            _ => {}
        }
    }

    // 필수 필드 확인
    let session_id = session_id.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "MISSING_SESSION_ID".to_string(),
            message: "session_id가 필요합니다".to_string(),
        }))
    })?;

    let audio_data = audio_data.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "MISSING_AUDIO".to_string(),
            message: "오디오 파일이 필요합니다".to_string(),
        }))
    })?;

    let audio_format = audio_format.unwrap_or(AudioFormat::Wav);

    // 세션 확인
    let mut agent = match state.get_session(&session_id).await {
        Some(agent) => agent,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "SESSION_NOT_FOUND".to_string(),
                    message: format!("세션 {}를 찾을 수 없습니다", session_id),
                }),
            ));
        }
    };

    // Whisper API로 음성 인식
    let whisper = OpenAIWhisper::from_env().map_err(|e| {
        error!(error = %e, "Whisper 초기화 실패");
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "WHISPER_INIT_ERROR".to_string(),
            message: format!("음성 인식 초기화 실패: {}", e),
        }))
    })?;

    info!(session_id = %session_id, size = audio_data.len(), "음성 인식 시작");

    let transcript = whisper.transcribe_bytes(&audio_data, audio_format, session_id).await
        .map_err(|e| {
            error!(error = %e, "음성 인식 실패");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "TRANSCRIPTION_ERROR".to_string(),
                message: format!("음성 인식 실패: {}", e),
            }))
        })?;

    info!(
        session_id = %session_id,
        text = %transcript.text,
        duration = transcript.duration_secs,
        "음성 인식 완료"
    );

    // 감정 데이터 변환
    let emotion = emotion_input.map(|e| {
        EmotionResult::new(
            e.to_emotion_type(),
            e.valence,
            e.arousal,
            "api_input",
            session_id,
        )
    });

    // VLA 파이프라인 실행
    let result = agent.process_text_input(&transcript.text, emotion);

    // 세션 업데이트
    state.update_session(&session_id, agent).await;

    let coherence_score = result.psych_state.coherence_score;

    Ok(Json(AudioAnalyzeResponse {
        session_id,
        transcript: transcript.text,
        transcript_segments: transcript.segments.into_iter().map(|s| TranscriptSegmentDto {
            start: s.start,
            end: s.end,
            text: s.text,
        }).collect(),
        language: transcript.language,
        duration_secs: transcript.duration_secs,
        triples: result.triples,
        psych_state: result.psych_state,
        coherence_score,
        contradiction_detected: result.contradiction_detected,
        alerts: result.alerts,
        graph_summary: result.graph_summary,
    }))
}


/// POST /api/v1/analyze/face
/// 
/// 이미지에서 표정 분석 → 감정 데이터 반환
pub async fn analyze_face(
    State(_state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<FaceAnalyzeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mut session_id: Option<Uuid> = None;
    let mut image_data: Option<Vec<u8>> = None;
    let mut content_type: String = "image/jpeg".to_string();

    // multipart 필드 파싱
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "MULTIPART_ERROR".to_string(),
            message: e.to_string(),
        }))
    })? {
        let name = field.name().unwrap_or("").to_string();
        
        match name.as_str() {
            "session_id" => {
                let text = field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error: "FIELD_ERROR".to_string(),
                        message: format!("session_id 읽기 실패: {}", e),
                    }))
                })?;
                session_id = Some(Uuid::parse_str(&text).map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error: "INVALID_UUID".to_string(),
                        message: format!("잘못된 session_id: {}", e),
                    }))
                })?);
            }
            "image" => {
                if let Some(ct) = field.content_type() {
                    content_type = ct.to_string();
                }
                
                image_data = Some(field.bytes().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error: "IMAGE_READ_ERROR".to_string(),
                        message: format!("이미지 읽기 실패: {}", e),
                    }))
                })?.to_vec());
            }
            _ => {}
        }
    }

    // 필수 필드 확인
    let session_id = session_id.unwrap_or_else(Uuid::new_v4);

    let image_data = image_data.ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(ErrorResponse {
            error: "MISSING_IMAGE".to_string(),
            message: "이미지 파일이 필요합니다".to_string(),
        }))
    })?;

    // Azure Face API로 표정 분석
    let face_api = AzureFaceApi::from_env().map_err(|e| {
        error!(error = %e, "Face API 초기화 실패");
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "FACE_API_INIT_ERROR".to_string(),
            message: format!("표정 분석 초기화 실패: {} (AZURE_FACE_ENDPOINT, AZURE_FACE_KEY 설정 필요)", e),
        }))
    })?;

    info!(session_id = %session_id, size = image_data.len(), "표정 분석 시작");

    let face_result = face_api.analyze_bytes(&image_data, &content_type, session_id).await
        .map_err(|e| {
            error!(error = %e, "표정 분석 실패");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "FACE_ANALYSIS_ERROR".to_string(),
                message: format!("표정 분석 실패: {}", e),
            }))
        })?;

    info!(
        session_id = %session_id,
        face_count = face_result.face_count,
        primary_emotion = ?face_result.primary_emotion,
        "표정 분석 완료"
    );

    // EmotionResult로 변환
    let emotion_result = face_result.to_emotion_result("face_api");

    Ok(Json(FaceAnalyzeResponse {
        session_id,
        face_count: face_result.face_count,
        faces: face_result.faces.into_iter().map(|f| FaceDto {
            face_id: f.face_id,
            bounding_box: BoundingBoxDto {
                x: f.bounding_box.x,
                y: f.bounding_box.y,
                width: f.bounding_box.width,
                height: f.bounding_box.height,
            },
            emotion: format!("{:?}", f.emotion),
            valence: f.valence,
            arousal: f.arousal,
            confidence: f.confidence,
        }).collect(),
        primary_emotion: face_result.primary_emotion.map(|e| format!("{:?}", e)),
        primary_valence: face_result.primary_valence,
        primary_arousal: face_result.primary_arousal,
        emotion_input: emotion_result.map(|e| super::dto::EmotionInput {
            emotion_type: format!("{:?}", e.emotion),
            valence: e.valence,
            arousal: e.arousal,
        }),
    }))
}



/// POST /api/v1/report/generate
/// 
/// Claude를 활용해 심리 분석 리포트 생성
pub async fn generate_report(
    State(state): State<AppState>,
    Json(req): Json<GenerateReportRequest>,
) -> Result<Json<PsychReportResponse>, (StatusCode, Json<ErrorResponse>)> {
    use crate::llm::PsychReporter;

    // 세션 확인
    let agent = match state.get_session(&req.session_id).await {
        Some(agent) => agent,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "SESSION_NOT_FOUND".to_string(),
                    message: format!("세션 {}를 찾을 수 없습니다", req.session_id),
                }),
            ));
        }
    };

    // 세션 상태 가져오기
    let (psych_state, _, _, _) = agent.get_session_stats();

    // PsychReporter 초기화
    let reporter = PsychReporter::from_env().map_err(|e| {
        error!(error = %e, "PsychReporter 초기화 실패");
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "REPORTER_INIT_ERROR".to_string(),
            message: format!("리포트 생성기 초기화 실패: {} (ANTHROPIC_API_KEY 설정 필요)", e),
        }))
    })?;

    info!(session_id = %req.session_id, "심리 리포트 생성 시작");

    // 리포트 생성 (triples와 alerts는 현재 세션에서 가져올 수 없으므로 빈 배열로)
    let report = reporter.generate_report(
        &psych_state,
        &[], // TODO: 세션에서 최근 triples 가져오기
        &[], // TODO: 세션에서 alerts 가져오기
        req.conversation_summary.as_deref(),
    ).await.map_err(|e| {
        error!(error = %e, "리포트 생성 실패");
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "REPORT_GENERATION_ERROR".to_string(),
            message: format!("리포트 생성 실패: {}", e),
        }))
    })?;

    info!(session_id = %req.session_id, risk_level = %report.risk_level, "리포트 생성 완료");

    Ok(Json(PsychReportResponse {
        session_id: req.session_id,
        report,
    }))
}

/// POST /api/v1/counselor/suggest
/// 
/// 상담사 추천 응답 생성
pub async fn suggest_counselor_response(
    State(state): State<AppState>,
    Json(req): Json<CounselorSuggestRequest>,
) -> Result<Json<CounselorSuggestionResponse>, (StatusCode, Json<ErrorResponse>)> {
    use crate::llm::PsychReporter;

    // 세션 확인
    let agent = match state.get_session(&req.session_id).await {
        Some(agent) => agent,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "SESSION_NOT_FOUND".to_string(),
                    message: format!("세션 {}를 찾을 수 없습니다", req.session_id),
                }),
            ));
        }
    };

    let (psych_state, _, _, _) = agent.get_session_stats();

    let reporter = PsychReporter::from_env().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "REPORTER_INIT_ERROR".to_string(),
            message: format!("초기화 실패: {}", e),
        }))
    })?;

    info!(session_id = %req.session_id, "상담사 추천 응답 생성 시작");

    let suggestion = reporter.suggest_response(
        &psych_state,
        &req.last_utterance,
        req.context.as_deref(),
    ).await.map_err(|e| {
        error!(error = %e, "추천 응답 생성 실패");
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
            error: "SUGGESTION_ERROR".to_string(),
            message: format!("추천 응답 생성 실패: {}", e),
        }))
    })?;

    Ok(Json(CounselorSuggestionResponse {
        session_id: req.session_id,
        suggestion,
    }))
}

