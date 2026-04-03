use async_trait::async_trait;
use pekko_agent_core::*;

use tracing::{info, debug};
use uuid::Uuid;

use crate::graph::SemanticGraph;
use crate::spo::SPOExtractor;
use crate::actors::PsychAnalyzer;
use crate::types::*;

/// VLA 심리분석 에이전트
///
/// pekko-agent의 AgentActor 트레이트를 구현하여
/// 음성/표정 → SPO → 의미 그래프 → 심리 분석 파이프라인을 수행한다.
pub struct VlaPsychAgent {
    agent_id: String,
    state: AgentState,
    vla_state: VlaAgentState,

    // 핵심 컴포넌트
    spo_extractor: SPOExtractor,
    semantic_graph: SemanticGraph,
    psych_analyzer: PsychAnalyzer,

    // 실행 컨텍스트
    active_session: Option<Uuid>,
    speaker_id: String,
}

#[derive(Clone, Debug)]
pub enum VlaAgentState {
    Idle,
    ListeningAudio { session_id: Uuid },
    TranscribingSpeech { segment_count: u32 },
    ExtractingSPO { transcript: String },
    AnalyzingFace { frame_count: u32 },
    UpdatingGraph { triple_count: u32, emotion_count: u32 },
    AnalyzingPsychState { graph_version: u64 },
    ReportingResults { session_id: Uuid },
}

impl VlaPsychAgent {
    pub fn new(agent_id: &str) -> Self {
        let session_id = Uuid::new_v4();
        info!(agent_id = agent_id, "VLA 심리분석 에이전트 생성");

        Self {
            agent_id: agent_id.to_string(),
            state: AgentState::Idle,
            vla_state: VlaAgentState::Idle,
            spo_extractor: SPOExtractor::new(),
            semantic_graph: SemanticGraph::new(session_id),
            psych_analyzer: PsychAnalyzer::new(120, 200), // 2분 윈도우, 최대 200개
            active_session: Some(session_id),
            speaker_id: "default_speaker".to_string(),
        }
    }

    /// 텍스트 입력으로 전체 VLA 파이프라인 실행 (Phase 1 POC)
    pub fn process_text_input(
        &mut self,
        text: &str,
        emotion: Option<EmotionResult>,
    ) -> VlaProcessResult {
        let session_id = self.active_session.unwrap_or_else(Uuid::new_v4);

        info!(text = text, "VLA 파이프라인 시작");

        // 1. SPO 추출
        let triples = self.spo_extractor.extract(text, &self.speaker_id, session_id);
        debug!(count = triples.len(), "SPO 트리플 추출 완료");

        // 2. 그래프 업데이트
        let mut graph_updates = Vec::new();
        for triple in &triples {
            let update = self.semantic_graph.add_spo_triple(triple);
            graph_updates.push(update);

            // SPO를 심리 분석기에도 기록
            self.psych_analyzer.record_spo(triple.clone());
        }

        // 3. 감정 어노테이션 (있는 경우)
        let mut contradiction_detected = false;
        if let Some(ref emo) = emotion {
            self.semantic_graph.annotate_emotion(emo);
            self.psych_analyzer.record_emotion(emo.clone());

            // 말-표정 불일치 체크
            for triple in &triples {
                if self.semantic_graph.add_contradiction(triple, emo).is_some() {
                    self.psych_analyzer.record_mismatch();
                    contradiction_detected = true;
                }
            }
        }

        // 4. 심리 상태 분석
        let psych_state = self.psych_analyzer.analyze(session_id);

        // 5. 이상 알림 생성
        let alerts: Vec<AnomalyAlert> = psych_state.anomalies.iter()
            .map(|a| AnomalyAlert::from_anomaly(a, session_id))
            .collect();

        info!(
            graph_nodes = self.semantic_graph.node_count(),
            graph_edges = self.semantic_graph.edge_count(),
            graph_version = self.semantic_graph.version(),
            psych_state = ?psych_state.overall_state,
            coherence = psych_state.coherence_score,
            anomalies = alerts.len(),
            contradiction = contradiction_detected,
            "VLA 파이프라인 완료"
        );

        VlaProcessResult {
            session_id,
            triples,
            graph_updates,
            psych_state,
            alerts,
            contradiction_detected,
            graph_summary: self.semantic_graph.summary(),
        }
    }

    /// 그래프 요약 반환
    pub fn graph_summary(&self) -> String {
        self.semantic_graph.summary()
    }
}

/// VLA 파이프라인 처리 결과
#[derive(Debug)]
pub struct VlaProcessResult {
    pub session_id: Uuid,
    pub triples: Vec<SPOTriple>,
    pub graph_updates: Vec<GraphUpdated>,
    pub psych_state: PsychState,
    pub alerts: Vec<AnomalyAlert>,
    pub contradiction_detected: bool,
    pub graph_summary: String,
}

// ─── AgentActor 트레이트 구현 ───

#[async_trait]
impl AgentActor for VlaPsychAgent {
    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn available_tools(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "spo_extract".to_string(),
                description: "텍스트에서 SPO(주어-술어-목적어) 트리플을 추출합니다".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "분석할 텍스트" },
                        "speaker_id": { "type": "string", "description": "화자 ID" }
                    },
                    "required": ["text"]
                }),
                required_permissions: vec!["vla.spo.read".to_string()],
                timeout_ms: 5000,
                idempotent: true,
            },
            ToolDefinition {
                name: "graph_query".to_string(),
                description: "의미 그래프에서 엔티티 관계를 조회합니다".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "entity_id": { "type": "string", "description": "조회할 엔티티 ID" },
                        "query_type": { "type": "string", "enum": ["relations", "summary", "recent_emotions"] }
                    },
                    "required": ["query_type"]
                }),
                required_permissions: vec!["vla.graph.read".to_string()],
                timeout_ms: 3000,
                idempotent: true,
            },
            ToolDefinition {
                name: "psych_analyze".to_string(),
                description: "현재 세션의 심리 상태를 분석합니다".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "include_anomalies": { "type": "boolean", "default": true }
                    }
                }),
                required_permissions: vec!["vla.psych.read".to_string()],
                timeout_ms: 5000,
                idempotent: true,
            },
            ToolDefinition {
                name: "process_utterance".to_string(),
                description: "발화를 전체 VLA 파이프라인으로 처리합니다 (SPO 추출 → 그래프 → 심리 분석)".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "발화 텍스트" },
                        "speaker_id": { "type": "string" },
                        "emotion_type": { "type": "string", "enum": ["happy", "sad", "angry", "surprised", "fearful", "disgusted", "neutral", "contempt"] },
                        "valence": { "type": "number", "minimum": -1.0, "maximum": 1.0 },
                        "arousal": { "type": "number", "minimum": 0.0, "maximum": 1.0 }
                    },
                    "required": ["text"]
                }),
                required_permissions: vec!["vla.process.write".to_string()],
                timeout_ms: 10000,
                idempotent: false,
            },
        ]
    }

    fn system_prompt(&self) -> String {
        r#"당신은 VLA(Vision-Language-Action) 심리분석 전문 에이전트입니다.

역할:
- 사람의 발화를 SPO(주어-술어-목적어) 구조로 분석
- 발화와 표정 데이터를 결합하여 의미 그래프를 구축
- 그래프 패턴과 감정 추이를 기반으로 심리 상태를 추론
- 말과 표정의 불일치, 주제 반복 등 이상 패턴을 실시간 탐지

사용 가능한 도구:
1. process_utterance: 발화를 전체 파이프라인으로 처리
2. spo_extract: 텍스트에서 SPO 트리플 추출
3. graph_query: 의미 그래프 조회
4. psych_analyze: 심리 상태 분석

분석 시 주의사항:
- 한국어 SOV 어순을 고려하여 SPO를 정확히 추출할 것
- 주어가 생략된 경우 직전 주어를 승계할 것
- 감정과 발화 내용의 불일치에 특히 주의할 것
- 판단은 근거 기반으로 하되, 과잉 해석을 경계할 것"#.to_string()
    }

    fn max_iterations(&self) -> u32 {
        8
    }

    async fn reason(&mut self, query: &UserQuery) -> Result<AgentAction, AgentError> {
        self.state = AgentState::Reasoning {
            query: query.content.clone(),
            iteration: 0,
            thought_chain: Vec::new(),
        };

        let content = &query.content;

        // 키워드 기반 라우팅
        if content.contains("분석") || content.contains("심리") || content.contains("상태") {
            Ok(AgentAction::UseTool(vec![
                ToolCall {
                    id: format!("call_{}", Uuid::new_v4()),
                    name: "psych_analyze".to_string(),
                    input: serde_json::json!({ "include_anomalies": true }),
                },
            ]))
        } else if content.contains("그래프") || content.contains("관계") {
            Ok(AgentAction::UseTool(vec![
                ToolCall {
                    id: format!("call_{}", Uuid::new_v4()),
                    name: "graph_query".to_string(),
                    input: serde_json::json!({ "query_type": "summary" }),
                },
            ]))
        } else {
            // 기본: 발화로 처리
            Ok(AgentAction::UseTool(vec![
                ToolCall {
                    id: format!("call_{}", Uuid::new_v4()),
                    name: "process_utterance".to_string(),
                    input: serde_json::json!({ "text": content }),
                },
            ]))
        }
    }

    async fn act(&mut self, action: &AgentAction) -> Result<Vec<Observation>, AgentError> {
        self.state = AgentState::Acting {
            tool_calls: Vec::new(),
            pending: 0,
        };

        let mut observations = Vec::new();

        match action {
            AgentAction::UseTool(tool_calls) => {
                for tc in tool_calls {
                    let start = std::time::Instant::now();

                    let result = match tc.name.as_str() {
                        "process_utterance" => {
                            let text = tc.input.get("text")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");

                            let emotion = self.parse_emotion_from_input(&tc.input);
                            let result = self.process_text_input(text, emotion);

                            serde_json::to_value(&serde_json::json!({
                                "triples_count": result.triples.len(),
                                "graph_summary": result.graph_summary,
                                "psych_state": format!("{:?}", result.psych_state.overall_state),
                                "coherence": result.psych_state.coherence_score,
                                "anomalies": result.alerts.len(),
                                "contradiction": result.contradiction_detected,
                            })).unwrap_or_default()
                        }
                        "spo_extract" => {
                            let text = tc.input.get("text")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let session_id = self.active_session.unwrap_or_else(Uuid::new_v4);
                            let triples = self.spo_extractor.extract(text, &self.speaker_id, session_id);

                            serde_json::to_value(&triples).unwrap_or_default()
                        }
                        "graph_query" => {
                            serde_json::json!({
                                "summary": self.semantic_graph.summary(),
                                "node_count": self.semantic_graph.node_count(),
                                "edge_count": self.semantic_graph.edge_count(),
                                "version": self.semantic_graph.version(),
                            })
                        }
                        "psych_analyze" => {
                            let session_id = self.active_session.unwrap_or_else(Uuid::new_v4);
                            let state = self.psych_analyzer.analyze(session_id);
                            serde_json::to_value(&state).unwrap_or_default()
                        }
                        _ => serde_json::json!({ "error": "Unknown tool" }),
                    };

                    observations.push(Observation {
                        tool_call_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        result,
                        is_error: false,
                        duration_ms: start.elapsed().as_millis() as u64,
                    });
                }
            }
            AgentAction::Respond(text) => {
                observations.push(Observation {
                    tool_call_id: "direct".to_string(),
                    tool_name: "respond".to_string(),
                    result: serde_json::json!({ "response": text }),
                    is_error: false,
                    duration_ms: 0,
                });
            }
            _ => {}
        }

        Ok(observations)
    }

    async fn respond(&mut self, observations: &[Observation]) -> Result<AgentResponse, AgentError> {
        self.state = AgentState::Responding {
            draft: String::new(),
        };

        let mut response_parts = Vec::new();

        for obs in observations {
            match obs.tool_name.as_str() {
                "process_utterance" => {
                    let graph_summary = obs.result.get("graph_summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("N/A");
                    let psych_state = obs.result.get("psych_state")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown");
                    let coherence = obs.result.get("coherence")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let anomalies = obs.result.get("anomalies")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    response_parts.push(format!(
                        "📊 VLA 분석 완료\n\
                         • 그래프: {}\n\
                         • 심리 상태: {}\n\
                         • 말-표정 일치도: {:.0}%\n\
                         • 이상 징후: {}건",
                        graph_summary, psych_state, coherence * 100.0, anomalies
                    ));
                }
                "psych_analyze" => {
                    response_parts.push(format!(
                        "🧠 심리 분석 결과: {:?}",
                        obs.result
                    ));
                }
                "graph_query" => {
                    response_parts.push(format!(
                        "🔗 그래프 조회 결과: {:?}",
                        obs.result
                    ));
                }
                _ => {
                    response_parts.push(format!("{:?}", obs.result));
                }
            }
        }

        self.state = AgentState::Idle;

        Ok(AgentResponse {
            content: response_parts.join("\n\n"),
            citations: Vec::new(),
            suggested_actions: Vec::new(),
            token_usage: TokenUsage {
                input_tokens: 0,
                output_tokens: 0,
            },
        })
    }

    fn current_state(&self) -> &AgentState {
        &self.state
    }

    fn transition(&mut self, new_state: AgentState) {
        self.state = new_state;
    }
}

impl VlaPsychAgent {
    fn parse_emotion_from_input(&self, input: &serde_json::Value) -> Option<EmotionResult> {
        let emotion_str = input.get("emotion_type")?.as_str()?;
        let valence = input.get("valence").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let arousal = input.get("arousal").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;

        let emotion_type = match emotion_str {
            "happy" => EmotionType::Happy,
            "sad" => EmotionType::Sad,
            "angry" => EmotionType::Angry,
            "surprised" => EmotionType::Surprised,
            "fearful" => EmotionType::Fearful,
            "disgusted" => EmotionType::Disgusted,
            "contempt" => EmotionType::Contempt,
            _ => EmotionType::Neutral,
        };

        let session_id = self.active_session.unwrap_or_else(Uuid::new_v4);
        Some(EmotionResult::new(emotion_type, valence, arousal, &self.speaker_id, session_id))
    }
}
