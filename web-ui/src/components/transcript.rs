use leptos::prelude::*;
use crate::app::TranscriptEntry;
use crate::components::icons;

#[component]
pub fn Transcript(
    is_active: ReadSignal<bool>,
    case_id: ReadSignal<Option<String>>,
    transcripts: ReadSignal<Vec<TranscriptEntry>>,
    #[allow(unused)]
    set_transcripts: WriteSignal<Vec<TranscriptEntry>>,
) -> impl IntoView {
    // WebSocket 연결
    Effect::new(move |_| {
        if is_active.get() {
            if let Some(cid) = case_id.get() {
                log::info!("Connecting to case: {}", cid);
            }
        }
    });
    
    view! {
        <div class="grok-card rounded-md p-4 h-full flex flex-col">
            <h2 class="text-xs font-medium text-grok-muted uppercase tracking-wider flex items-center gap-2 mb-3">
                {icons::message_square()}
                "Conversation Log"
            </h2>
            
            <div class="flex-1 overflow-y-auto space-y-2 min-h-[300px] max-h-[500px]">
                {move || {
                    let entries = transcripts.get();
                    if entries.is_empty() {
                        view! {
                            <div class="flex flex-col items-center justify-center h-full py-12">
                                <div class="text-grok-muted mb-2">
                                    {icons::mic()}
                                </div>
                                <p class="text-xs text-grok-muted font-mono">"Waiting for conversation..."</p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="space-y-2">
                                {entries.into_iter().map(|entry| {
                                    let is_analyst = entry.speaker == "analyst" || entry.speaker == "counselor";
                                    
                                    view! {
                                        <div class=if is_analyst { "flex justify-end" } else { "flex justify-start" }>
                                            <div class=if is_analyst {
                                                "max-w-[85%] px-3 py-2 bg-grok-blue text-white text-sm rounded"
                                            } else {
                                                "max-w-[85%] px-3 py-2 bg-grok-gray text-white text-sm rounded border border-grok-border"
                                            }>
                                                <p class="text-[10px] text-white/50 mb-0.5 font-mono uppercase tracking-wider">
                                                    {if is_analyst { "analyst" } else { "client" }}
                                                </p>
                                                <p class="leading-relaxed">{entry.text.clone()}</p>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_any()
                    }
                }}
            </div>
            
            // 실시간 입력 표시
            <Show when=move || is_active.get()>
                <div class="mt-3 pt-3 border-t border-grok-border">
                    <div class="flex items-center gap-2 text-grok-muted">
                        <div class="flex gap-0.5">
                            <div class="w-1 h-1 bg-grok-blue rounded-full animate-bounce" style="animation-delay: 0ms" />
                            <div class="w-1 h-1 bg-grok-blue rounded-full animate-bounce" style="animation-delay: 100ms" />
                            <div class="w-1 h-1 bg-grok-blue rounded-full animate-bounce" style="animation-delay: 200ms" />
                        </div>
                        <span class="text-xs font-mono">"listening"</span>
                    </div>
                </div>
            </Show>
        </div>
    }
}
