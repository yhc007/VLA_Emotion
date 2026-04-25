use leptos::prelude::*;
use wasm_bindgen::prelude::*;
// use crate::api::client;  // TODO: 백엔드 연결 시 활성화
use crate::components::icons;

/// 다음 프레임에서 콜백 실행 (cleanup이 먼저 완료되도록)
fn request_animation_frame(f: impl FnOnce() + 'static) {
    let closure = Closure::once_into_js(f);
    web_sys::window()
        .expect("no window")
        .request_animation_frame(closure.as_ref().unchecked_ref())
        .expect("requestAnimationFrame failed");
}

#[component]
pub fn SessionControl(
    case_id: ReadSignal<Option<String>>,
    is_active: ReadSignal<bool>,
    set_case_id: WriteSignal<Option<String>>,
    set_is_active: WriteSignal<bool>,
    set_show_report: WriteSignal<bool>,
) -> impl IntoView {
    let start_case = move |_| {
        // 이전 케이스가 활성 상태이면 먼저 종료
        if is_active.get() {
            set_is_active.set(false);
            log::info!("Previous case deactivated before starting new one");
        }

        // Mock 모드: API 없이 로컬에서 바로 시작
        let mock_id = format!("{:08x}", js_sys::Date::now() as u64);
        set_case_id.set(Some(mock_id.clone()));
        // requestAnimationFrame 틱 후 활성화하여 cleanup이 먼저 완료되도록 함
        request_animation_frame(move || {
            set_is_active.set(true);
            log::info!("Case started (mock): {}", mock_id);
        });
        
        // TODO: 백엔드 연결 시 아래 코드 활성화
        // wasm_bindgen_futures::spawn_local(async move {
        //     match client::create_session().await {
        //         Ok(id) => {
        //             set_case_id.set(Some(id));
        //             set_is_active.set(true);
        //             log::info!("Case started");
        //         }
        //         Err(e) => log::error!("Failed to start case: {}", e),
        //     }
        // });
    };
    
    let end_case = move |_| {
        // Mock 모드: API 없이 로컬에서 바로 종료
        if let Some(id) = case_id.get() {
            set_is_active.set(false);
            log::info!("Case ended (mock): {}", id);
        }
        
        // TODO: 백엔드 연결 시 아래 코드 활성화
        // if let Some(id) = case_id.get() {
        //     wasm_bindgen_futures::spawn_local(async move {
        //         match client::end_session(&id).await {
        //             Ok(_) => {
        //                 set_is_active.set(false);
        //                 log::info!("Case ended");
        //             }
        //             Err(e) => log::error!("Failed to end case: {}", e),
        //         }
        //     });
        // }
    };
    
    let view_report = move |_| {
        set_show_report.set(true);
    };
    
    view! {
        <div class="grok-card rounded-md p-4">
            <div class="flex items-center justify-between">
                // Case 상태
                <div class="flex items-center gap-4">
                    <div class=move || {
                        if is_active.get() {
                            "w-2 h-2 rounded-full bg-green-500 animate-pulse"
                        } else {
                            "w-2 h-2 rounded-full bg-grok-muted"
                        }
                    } />
                    <div>
                        <p class="text-sm font-medium text-white">
                            {move || if is_active.get() { "Analyzing" } else { "Ready" }}
                        </p>
                        <p class="text-xs text-grok-muted font-mono">
                            {move || case_id.get().map(|id| format!("CASE-{}", &id[..6.min(id.len())].to_uppercase())).unwrap_or_else(|| "—".to_string())}
                        </p>
                    </div>
                </div>
                
                // 버튼들
                <div class="flex gap-2">
                    <Show when=move || !is_active.get()>
                        <button
                            on:click=start_case
                            class="grok-btn flex items-center gap-2 px-4 py-2 bg-grok-blue hover:bg-grok-blue-hover text-white text-sm font-medium rounded-md"
                        >
                            {icons::play()}
                            "New Case"
                        </button>
                    </Show>
                    
                    <Show when=move || is_active.get()>
                        <button
                            on:click=end_case
                            class="grok-btn flex items-center gap-2 px-4 py-2 bg-red-600 hover:bg-red-500 text-white text-sm font-medium rounded-md"
                        >
                            {icons::stop()}
                            "End"
                        </button>
                    </Show>
                    
                    <Show when=move || case_id.get().is_some() && !is_active.get()>
                        <button
                            on:click=view_report
                            class="grok-btn flex items-center gap-2 px-4 py-2 bg-grok-gray hover:bg-grok-border text-white text-sm font-medium rounded-md border border-grok-border"
                        >
                            {icons::bar_chart()}
                            "Solution"
                        </button>
                    </Show>
                </div>
            </div>
        </div>
    }
}
