//! VLA 심리분석 에이전트
//!
//! 실행 모드:
//! - 서버 모드 (기본): `cargo run`
//! - 데모 모드: `cargo run -- --demo`

mod types;
mod spo;
mod graph;
mod actors;
mod api;
mod speech;
mod vision;
mod llm;

use actors::VlaPsychAgent;
use types::*;
use pekko_agent_core::*;
use pekko_agent_events::schema::AgentEventEnvelope;
use pekko_agent_events::publisher::EventPublisher;
use std::collections::HashMap;
use std::net::SocketAddr;
use tracing::{info, error, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq)]
enum RunMode {
    Server,
    Demo,
}

fn parse_args() -> RunMode {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--demo") {
        RunMode::Demo
    } else {
        RunMode::Server
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 로깅 초기화
    tracing_subscriber::fmt()
        .with_env_filter("info,vla_psych_agent=debug,tower_http=debug")
        .with_target(false)
        .with_level(true)
        .init();

    let mode = parse_args();

    match mode {
        RunMode::Server => run_server().await,
        RunMode::Demo => run_demo().await,
    }
}

/// HTTP API 서버 모드
async fn run_server() -> anyhow::Result<()> {
    info!("═══════════════════════════════════════════════════════");
    info!("  🧠 VLA 심리분석 에이전트 — HTTP API 서버");
    info!("═══════════════════════════════════════════════════════");

    let state = api::AppState::new();
    let app = api::create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("🚀 서버 시작: http://{}", addr);
    info!("");
    info!("📡 엔드포인트:");
    info!("   GET  /health                    - 헬스 체크");
    info!("   POST /api/v1/session/start      - 세션 시작");
    info!("   POST /api/v1/analyze/text       - 텍스트 분석");
    info!("   POST /api/v1/analyze/audio      - 🎤 음성 분석 + VLA (Whisper + SPO)");
    info!("   POST /api/stt                   - 🎤 순수 STT (Whisper, 프론트 실시간 녹음용)");
    info!("   POST /api/v1/analyze/face       - 😊 표정 분석 (Azure Face)");
    info!("   POST /api/v1/report/generate    - 🧠 AI 심리 리포트 (Claude)");
    info!("   POST /api/v1/counselor/suggest  - 💬 상담사 추천 응답 (Claude)");
    info!("   GET  /api/v1/session/:id/state  - 심리 상태 조회");
    info!("   GET  /api/v1/session/:id/graph  - 그래프 조회");
    info!("");
    info!("🔌 WebSocket:");
    info!("   WS   /ws                        - 새 세션 + 실시간 스트리밍");
    info!("   WS   /ws/:id                    - 기존 세션 연결");
    info!("");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// 데모 모드 (기존 시뮬레이션)
async fn run_demo() -> anyhow::Result<()> {
    info!("═══════════════════════════════════════════════════════");
    info!("  🧠 VLA 심리분석 에이전트 — 데모 모드");
    info!("  Phase 1 POC — 텍스트 → SPO → 그래프 → 심리 분석");
    info!("═══════════════════════════════════════════════════════");

    // 이벤트 버스 초기화 (pekko-event-bus)
    let event_publisher = EventPublisher::new("vla", 256);
    info!("pekko-event-bus 초기화 완료 (topic: vla)");

    // VLA 에이전트 생성
    let mut agent = VlaPsychAgent::new("vla-psych-001");

    // ─── 데모: 상담 시나리오 시뮬레이션 ───

    info!("\n📝 [데모] 상담 시나리오 시뮬레이션\n");

    let demo_utterances = vec![
        // (텍스트, 감정, valence, arousal)
        ("나는 요즘 회사가 너무 힘들어", Some(("sad", -0.6, 0.4)),
         "첫 발화: 부정적 내용 + 슬픈 표정 (일치)"),
        ("상사가 나를 무시해", Some(("angry", -0.7, 0.7)),
         "분노 표출: 특정 인물 언급"),
        ("동료들은 좋은데 분위기가 안 좋아", Some(("sad", -0.4, 0.3)),
         "혼합 감정: 긍정/부정 공존"),
        ("사실 나는 괜찮아", Some(("sad", -0.5, 0.3)),
         "⚠️ 말-표정 불일치: 긍정적 발화 + 부정적 표정"),
        ("회사를 그만둘까 생각중이야", Some(("fearful", -0.6, 0.6)),
         "중요 의사결정 언급"),
    ];

    for (i, (text, emotion_data, description)) in demo_utterances.iter().enumerate() {
        info!("────────────────────────────────────────");
        info!("발화 #{}: \"{}\"", i + 1, text);
        info!("설명: {}", description);

        // 감정 데이터 생성
        let emotion = emotion_data.map(|(emo_type, valence, arousal)| {
            let emo = match emo_type {
                "happy" => EmotionType::Happy,
                "sad" => EmotionType::Sad,
                "angry" => EmotionType::Angry,
                "surprised" => EmotionType::Surprised,
                "fearful" => EmotionType::Fearful,
                "disgusted" => EmotionType::Disgusted,
                "contempt" => EmotionType::Contempt,
                _ => EmotionType::Neutral,
            };
            EmotionResult::new(emo, valence as f32, arousal as f32, "speaker_01", Uuid::new_v4())
        });

        // VLA 파이프라인 실행
        let result = agent.process_text_input(text, emotion);

        // 결과 출력
        info!("  📊 SPO 트리플: {}개 추출", result.triples.len());
        for triple in &result.triples {
            info!(
                "     ({}) —[{} / {:?}]→ ({})",
                triple.subject.name,
                triple.predicate.verb,
                triple.predicate.polarity,
                triple.object.name,
            );
        }
        info!("  🔗 그래프: {}", result.graph_summary);
        info!("  🧠 심리 상태: {:?}", result.psych_state.overall_state);
        info!("  📏 일치도: {:.0}%", result.psych_state.coherence_score * 100.0);

        if result.contradiction_detected {
            warn!("  ⚠️ 말-표정 불일치 감지!");
        }

        for alert in &result.alerts {
            warn!(
                "  🚨 이상 탐지: {:?} (심각도: {:?}) — {}",
                alert.alert_type, alert.severity, alert.recommended_action
            );
        }

        // 이벤트 퍼블리시
        let event = AgentEventEnvelope::new(
            "vla-psych-agent",
            vla_event_types::GRAPH_UPDATED,
            "tenant-001",
            Uuid::new_v4(),
            serde_json::json!({
                "utterance": text,
                "spo_count": result.triples.len(),
                "psych_state": format!("{:?}", result.psych_state.overall_state),
                "coherence": result.psych_state.coherence_score,
            }),
        );
        if let Err(e) = event_publisher.publish(event).await {
            error!("이벤트 퍼블리시 실패: {}", e);
        }

        info!("");
    }

    // ─── AgentActor 트레이트 기반 쿼리 데모 ───

    info!("═══════════════════════════════════════════════════════");
    info!("📡 AgentActor 트레이트 기반 쿼리 데모\n");

    let queries = vec![
        "현재 심리 상태를 분석해줘",
        "그래프 관계를 보여줘",
    ];

    for query_text in queries {
        info!("Query: \"{}\"", query_text);

        let query = UserQuery {
            session_id: Uuid::new_v4(),
            content: query_text.to_string(),
            context: ConversationContext {
                messages: vec![],
                metadata: HashMap::new(),
            },
            auth: AuthContext {
                user_id: "user-vla-001".to_string(),
                tenant_id: "tenant-001".to_string(),
                roles: vec!["vla_analyst".to_string()],
            },
        };

        match agent.reason(&query).await {
            Ok(action) => {
                match agent.act(&action).await {
                    Ok(observations) => {
                        match agent.respond(&observations).await {
                            Ok(response) => {
                                info!("Response:\n{}\n", response.content);
                            }
                            Err(e) => error!("Respond 실패: {}", e),
                        }
                    }
                    Err(e) => error!("Act 실패: {}", e),
                }
            }
            Err(e) => error!("Reason 실패: {}", e),
        }
    }

    info!("═══════════════════════════════════════════════════════");
    info!("  ✅ VLA 심리분석 에이전트 데모 완료");
    info!("  최종 그래프: {}", agent.graph_summary());
    info!("═══════════════════════════════════════════════════════");

    Ok(())
}
