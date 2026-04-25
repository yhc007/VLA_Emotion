/// VLA 전용 이벤트 타입 상수
#[allow(dead_code)]
pub mod vla_event_types {
    pub const SPEECH_DETECTED: &str = "vla.speech.detected";
    pub const TRANSCRIPT_READY: &str = "vla.transcript.ready";
    pub const SPO_EXTRACTED: &str = "vla.spo.extracted";
    pub const EMOTION_DETECTED: &str = "vla.emotion.detected";
    pub const GRAPH_UPDATED: &str = "vla.graph.updated";
    pub const PSYCH_STATE_CHANGED: &str = "vla.psych.state_changed";
    pub const ANOMALY_DETECTED: &str = "vla.anomaly.detected";
    pub const SESSION_STARTED: &str = "vla.session.started";
    pub const SESSION_ENDED: &str = "vla.session.ended";
}
