use axum::{
    extract::{Path, State, Json},
    routing::{get, post, put},
    Router,
    http::StatusCode,
    response::IntoResponse,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tracing::info;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

use pekko_agent_core::{Message, TokenUsage, ShortTermMemory};
use pekko_agent_llm::LlmConfig;
use pekko_agent_tools::{ToolRegistry, builtin::{PermitSearchTool, ComplianceCheckTool}};
use pekko_agent_memory::{InMemoryConversationStore, InMemoryVectorStore, InMemoryEpisodicStore};
use pekko_agent_orchestrator::OrchestratorActor;
use pekko_agent_events::EventPublisher;
use pekko_agent_security::{RbacManager, TenantManager, AuditLogger};

/// Shared application state with all services
#[derive(Clone)]
struct AppState {
    tool_registry: Arc<RwLock<ToolRegistry>>,
    conversation_store: Arc<InMemoryConversationStore>,
    vector_store: Arc<InMemoryVectorStore>,
    episodic_store: Arc<InMemoryEpisodicStore>,
    orchestrator: Arc<RwLock<OrchestratorActor>>,
    event_publisher: Arc<EventPublisher>,
    rbac: Arc<RwLock<RbacManager>>,
    tenant_manager: Arc<RwLock<TenantManager>>,
    audit_logger: Arc<AuditLogger>,
    llm_config: LlmConfig,
}

/// Request payload for agent queries
#[derive(Deserialize)]
struct QueryRequest {
    content: String,
    #[serde(default)]
    session_id: Option<Uuid>,
    #[serde(default = "default_tenant")]
    tenant_id: String,
    #[serde(default = "default_user")]
    user_id: String,
}

fn default_tenant() -> String {
    "default".to_string()
}

fn default_user() -> String {
    "anonymous".to_string()
}

/// Response from agent query
#[derive(Serialize)]
struct QueryResponse {
    session_id: Uuid,
    agent_id: String,
    response: String,
    tools_used: Vec<String>,
    token_usage: TokenUsage,
}

/// Health check response
#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    services: ServiceStatus,
}

/// Status of individual services
#[derive(Serialize)]
struct ServiceStatus {
    orchestrator: String,
    tools_registered: usize,
    active_agents: usize,
}

/// Error response
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: String,
}

/// GET /api/health - Health check endpoint
async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let tools = state.tool_registry.read().await;
    let orch = state.orchestrator.read().await;

    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        services: ServiceStatus {
            orchestrator: "running".to_string(),
            tools_registered: tools.list_tools().len(),
            active_agents: orch.list_agents().len(),
        },
    })
}

/// GET /api/agents - List all available agents
async fn list_agents(State(state): State<AppState>) -> impl IntoResponse {
    let orch = state.orchestrator.read().await;
    let agents: Vec<_> = orch.list_agents().into_iter().cloned().collect();
    Json(agents)
}

/// POST /api/agents/:agent_id/query - Submit a query to an agent
async fn query_agent(
    Path(agent_id): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_id = req.session_id.unwrap_or_else(Uuid::new_v4);

    info!(
        agent_id = %agent_id,
        session_id = %session_id,
        content_len = req.content.len(),
        "Agent query received"
    );

    // Store the user message in conversation history
    let msg = Message::user(&req.content);
    let conv_store = state.conversation_store.clone();
    let _ = conv_store.append_message(&session_id, msg).await;

    // Publish task assigned event
    let event = pekko_agent_events::AgentEventEnvelope::new(
        "api-gateway",
        pekko_agent_events::event_types::TASK_ASSIGNED,
        &req.tenant_id,
        session_id,
        serde_json::json!({
            "agent_id": agent_id,
            "content": req.content,
        }),
    );
    let _ = state.event_publisher.publish(event).await;

    // Log audit entry
    state.audit_logger.log(pekko_agent_security::AuditEntry {
        id: Uuid::new_v4(),
        timestamp: chrono::Utc::now(),
        tenant_id: req.tenant_id.clone(),
        agent_id: agent_id.clone(),
        action: "query".to_string(),
        resource: format!("agent/{}", agent_id),
        outcome: pekko_agent_security::AuditOutcome::Success,
        details: serde_json::json!({"session_id": session_id}),
    }).await;

    // In production, this would invoke the actual agent via gRPC
    // For now, return a structured response
    let response_text = format!(
        "Agent '{}' received your query. Session: {}. \
         In production, this routes to the agent's ReAct loop via gRPC.",
        agent_id, session_id
    );

    // Store the assistant response
    let assistant_msg = Message::assistant(&response_text);
    let _ = conv_store.append_message(&session_id, assistant_msg).await;

    Ok(Json(QueryResponse {
        session_id,
        agent_id,
        response: response_text,
        tools_used: vec![],
        token_usage: TokenUsage::default(),
    }))
}

/// GET /api/sessions/:session_id/history - Get conversation history for a session
async fn get_session_history(
    Path(session_id): Path<Uuid>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let messages = state.conversation_store
        .get_conversation(&session_id)
        .await
        .unwrap_or_default();
    Json(messages)
}

/// GET /api/tools - List all available tools
async fn list_tools(State(state): State<AppState>) -> impl IntoResponse {
    let registry = state.tool_registry.read().await;
    Json(registry.list_tools())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing with JSON formatting
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .json()
        .init();

    info!("Starting pekko-agent API Gateway");

    // Initialize tool registry with built-in tools
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(Arc::new(PermitSearchTool));
    tool_registry.register(Arc::new(ComplianceCheckTool));

    // Create application state
    let state = AppState {
        tool_registry: Arc::new(RwLock::new(tool_registry)),
        conversation_store: Arc::new(InMemoryConversationStore::new(100)),
        vector_store: Arc::new(InMemoryVectorStore::new()),
        episodic_store: Arc::new(InMemoryEpisodicStore::new()),
        orchestrator: Arc::new(RwLock::new(OrchestratorActor::new())),
        event_publisher: Arc::new(EventPublisher::new("pekko-agent", 1024)),
        rbac: Arc::new(RwLock::new(RbacManager::new())),
        tenant_manager: Arc::new(RwLock::new(TenantManager::new())),
        audit_logger: Arc::new(AuditLogger::new(10000)),
        llm_config: LlmConfig::default(),
    };

    // Build the router with all routes
    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/agents", get(list_agents))
        .route("/api/agents/:agent_id/query", post(query_agent))
        .route("/api/sessions/:session_id/history", get(get_session_history))
        .route("/api/tools", get(list_tools))
        // 데이터 저장소 라우트
        .route("/api/storage/sessions", post(create_session))
        .route("/api/storage/sessions/:session_id/transcript", post(append_transcript))
        .route("/api/storage/sessions/:session_id/sentiment", post(save_sentiment))
        .route("/api/storage/sessions/:session_id/reasoning", post(save_reasoning))
        .route("/api/storage/sessions/:session_id/metadata", put(update_metadata))
        .route("/api/storage/sessions/:session_id/end", post(end_session))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Bind to port and start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    info!("API Gateway listening on 0.0.0.0:8080");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Signal handler for graceful shutdown
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl+c");
    info!("Shutdown signal received, gracefully shutting down");
}

// === 데이터 저장소 구조체들 ===

/// 세션 메타데이터
#[derive(Serialize, Deserialize, Clone, Debug)]
struct SessionMetadata {
    session_id: String,
    start_time: DateTime<Utc>,
    end_time: Option<DateTime<Utc>>,
    participant_count: usize,
    total_utterances: usize,
    total_duration_seconds: f64,
    languages: Vec<String>,
    keywords: Vec<String>,
    sentiment_summary: SentimentSummary,
    problem_categories: Vec<String>,
}

/// 감정 요약 통계
#[derive(Serialize, Deserialize, Clone, Debug)]
struct SentimentSummary {
    dominant_sentiment: String,
    average_confidence: f64,
    sentiment_distribution: HashMap<String, f64>,
    emotional_volatility: f64,
}

/// 타임스탬프가 포함된 대화 엔트리
#[derive(Serialize, Deserialize, Clone, Debug)]
struct TranscriptEntryWithTimestamp {
    timestamp: DateTime<Utc>,
    text: String,
    speaker: String,
    confidence: Option<f64>,
    language: Option<String>,
    duration_ms: Option<u64>,
}

/// 타임스탬프가 포함된 감정 데이터
#[derive(Serialize, Deserialize, Clone, Debug)]
struct SentimentDataWithTimestamp {
    timestamp: DateTime<Utc>,
    sentiment: String,
    confidence: f64,
    text_analyzed: String,
}

/// 타임스탬프가 포함된 추론 결과
#[derive(Serialize, Deserialize, Clone, Debug)]
struct ReasoningResultWithTimestamp {
    timestamp: DateTime<Utc>,
    reasoning_result: String,
    trigger_text: String,
}

// === 저장소 핸들러들 ===

/// POST /api/storage/sessions - 새 세션 생성
async fn create_session(
    State(_state): State<AppState>,
    Json(metadata): Json<SessionMetadata>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let session_dir = format!("./data/raw/sessions/{}", metadata.session_id);
    
    // 디렉토리 생성
    if let Err(e) = tokio::fs::create_dir_all(&session_dir).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to create session directory: {}", e),
                code: "STORAGE_ERROR".to_string(),
            }),
        ));
    }
    
    // 메타데이터 파일 저장
    let metadata_path = format!("{}/metadata.json", session_dir);
    let metadata_json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to serialize metadata: {}", e),
                    code: "SERIALIZATION_ERROR".to_string(),
                }),
            )
        })?;
    
    tokio::fs::write(&metadata_path, metadata_json).await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to write metadata: {}", e),
                    code: "STORAGE_ERROR".to_string(),
                }),
            )
        })?;
    
    info!("Session {} created successfully", metadata.session_id);
    
    Ok(Json(serde_json::json!({
        "status": "success",
        "session_id": metadata.session_id
    })))
}

/// POST /api/storage/sessions/:session_id/transcript - 대화 엔트리 추가
async fn append_transcript(
    Path(session_id): Path<String>,
    State(_state): State<AppState>,
    Json(entry): Json<TranscriptEntryWithTimestamp>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let transcript_path = format!("./data/raw/sessions/{}/transcript.jsonl", session_id);
    
    let line = serde_json::to_string(&entry)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to serialize transcript: {}", e),
                    code: "SERIALIZATION_ERROR".to_string(),
                }),
            )
        })?;
    
    // JSONL 형식으로 한 줄씩 추가
    let line_with_newline = format!("{}\n", line);
    tokio::fs::write(&transcript_path, 
        if tokio::fs::metadata(&transcript_path).await.is_ok() {
            let existing = tokio::fs::read_to_string(&transcript_path).await.unwrap_or_default();
            format!("{}{}", existing, line_with_newline)
        } else {
            line_with_newline
        }
    ).await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to write transcript: {}", e),
                code: "STORAGE_ERROR".to_string(),
            }),
        )
    })?;
    
    info!("Transcript appended to session {}: {} - {}", session_id, entry.speaker, entry.text);
    
    Ok(Json(serde_json::json!({
        "status": "success"
    })))
}

/// POST /api/storage/sessions/:session_id/sentiment - 감정 분석 결과 저장
async fn save_sentiment(
    Path(session_id): Path<String>,
    State(_state): State<AppState>,
    Json(entry): Json<SentimentDataWithTimestamp>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let sentiment_path = format!("./data/raw/sessions/{}/sentiment.jsonl", session_id);
    
    let line = serde_json::to_string(&entry)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to serialize sentiment: {}", e),
                    code: "SERIALIZATION_ERROR".to_string(),
                }),
            )
        })?;
    
    let line_with_newline = format!("{}\n", line);
    tokio::fs::write(&sentiment_path,
        if tokio::fs::metadata(&sentiment_path).await.is_ok() {
            let existing = tokio::fs::read_to_string(&sentiment_path).await.unwrap_or_default();
            format!("{}{}", existing, line_with_newline)
        } else {
            line_with_newline
        }
    ).await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to write sentiment: {}", e),
                code: "STORAGE_ERROR".to_string(),
            }),
        )
    })?;
    
    info!("Sentiment saved to session {}: {} ({})", session_id, entry.sentiment, entry.confidence);
    
    Ok(Json(serde_json::json!({
        "status": "success"
    })))
}

/// POST /api/storage/sessions/:session_id/reasoning - 추론 결과 저장
async fn save_reasoning(
    Path(session_id): Path<String>,
    State(_state): State<AppState>,
    Json(entry): Json<ReasoningResultWithTimestamp>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let reasoning_path = format!("./data/raw/sessions/{}/reasoning.jsonl", session_id);
    
    let line = serde_json::to_string(&entry)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to serialize reasoning: {}", e),
                    code: "SERIALIZATION_ERROR".to_string(),
                }),
            )
        })?;
    
    let line_with_newline = format!("{}\n", line);
    tokio::fs::write(&reasoning_path,
        if tokio::fs::metadata(&reasoning_path).await.is_ok() {
            let existing = tokio::fs::read_to_string(&reasoning_path).await.unwrap_or_default();
            format!("{}{}", existing, line_with_newline)
        } else {
            line_with_newline
        }
    ).await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to write reasoning: {}", e),
                code: "STORAGE_ERROR".to_string(),
            }),
        )
    })?;
    
    info!("Reasoning saved to session {}: {}", session_id, entry.trigger_text);
    
    Ok(Json(serde_json::json!({
        "status": "success"
    })))
}

/// PUT /api/storage/sessions/:session_id/metadata - 메타데이터 업데이트
async fn update_metadata(
    Path(session_id): Path<String>,
    State(_state): State<AppState>,
    Json(metadata): Json<SessionMetadata>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let metadata_path = format!("./data/raw/sessions/{}/metadata.json", session_id);
    
    let metadata_json = serde_json::to_string_pretty(&metadata)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to serialize metadata: {}", e),
                    code: "SERIALIZATION_ERROR".to_string(),
                }),
            )
        })?;
    
    tokio::fs::write(&metadata_path, metadata_json).await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to write metadata: {}", e),
                    code: "STORAGE_ERROR".to_string(),
                }),
            )
        })?;
    
    info!("Metadata updated for session {}", session_id);
    
    Ok(Json(serde_json::json!({
        "status": "success"
    })))
}

/// POST /api/storage/sessions/:session_id/end - 세션 종료
async fn end_session(
    Path(session_id): Path<String>,
    State(_state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let metadata_path = format!("./data/raw/sessions/{}/metadata.json", session_id);
    
    // 기존 메타데이터 읽기
    if let Ok(existing_content) = tokio::fs::read_to_string(&metadata_path).await {
        if let Ok(mut metadata) = serde_json::from_str::<SessionMetadata>(&existing_content) {
            // 종료 시간 업데이트
            metadata.end_time = Some(Utc::now());
            
            // 메타데이터 저장
            let metadata_json = serde_json::to_string_pretty(&metadata)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to serialize metadata: {}", e),
                            code: "SERIALIZATION_ERROR".to_string(),
                        }),
                    )
                })?;
            
            tokio::fs::write(&metadata_path, metadata_json).await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to write metadata: {}", e),
                            code: "STORAGE_ERROR".to_string(),
                        }),
                    )
                })?;
        }
    }
    
    info!("Session {} ended successfully", session_id);
    
    Ok(Json(serde_json::json!({
        "status": "success",
        "session_id": session_id
    })))
}
