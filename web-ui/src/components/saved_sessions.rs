// STUB: 본 구현이 복원되기 전까지 컴파일을 통과시키는 최소 인터페이스.
// 패널은 표시되지만 세션 로딩 기능은 동작하지 않는다.
//
// 본 구현 복원 시 다음 동작이 필요:
// - 서버 /sessions API 호출 → 세션 목록 표시
// - 항목 클릭 → /sessions/{id}/data 호출 → LoadedSessionResult로 변환 → on_load 발화

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use crate::app::{TranscriptEntry, SentimentData};
use crate::components::transcript::ReasoningResult;
use crate::components::timeline::TimelineMarker;

/// 서버 /sessions 응답 - 본 구현에서 필드 채워짐.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SavedSessionSummary {
    #[serde(default)] pub session_id: String,
    #[serde(default)] pub created_at: String,
    #[serde(default)] pub duration_ms: u64,
    #[serde(default)] pub transcript_count: usize,
}

/// 서버 /sessions/{id}/data 응답.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SavedSessionData {
    #[serde(default)] pub session_id: String,
    #[serde(default)] pub transcripts: Vec<serde_json::Value>,
    #[serde(default)] pub sentiments: Vec<serde_json::Value>,
}

/// 세션 로드 결과 - app.rs Effect가 set_loaded_session을 통해 소비.
#[derive(Debug, Clone, Default)]
pub struct LoadedSessionResult {
    pub session_id: String,
    pub transcripts: Vec<TranscriptEntry>,
    pub sentiments: Vec<SentimentData>,
    pub reasoning: Option<ReasoningResult>,
    pub timeline_markers: Vec<TimelineMarker>,
    pub duration_ms: u64,
}

#[component]
pub fn SavedSessions(
    on_load: WriteSignal<Option<LoadedSessionResult>>,
    on_close: Callback<()>,
) -> impl IntoView {
    // on_load은 본 구현 복원 시 세션 선택 핸들러에서 발화.
    let _ = on_load;
    view! {
        <div class="fixed inset-0 bg-black/70 flex items-center justify-center z-50">
            <div class="bg-grok-gray rounded-lg p-6 max-w-md w-full text-white border border-grok-border">
                <h2 class="text-lg font-medium mb-3">"저장된 세션"</h2>
                <p class="text-sm text-grok-muted">
                    "이 패널은 stub입니다. 본 구현이 복원되면 세션 목록과 리플레이가 활성화됩니다."
                </p>
                <button
                    class="mt-4 px-3 py-1 bg-grok-blue text-white rounded text-sm"
                    on:click=move |_| on_close.run(())
                >
                    "닫기"
                </button>
            </div>
        </div>
    }
}
