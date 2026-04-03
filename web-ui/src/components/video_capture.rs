use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlVideoElement, MediaStreamConstraints};
use crate::app::SentimentData;
use crate::components::icons;

#[component]
pub fn VideoCapture(
    is_active: ReadSignal<bool>,
    case_id: ReadSignal<Option<String>>,
    #[allow(unused)]
    set_sentiments: WriteSignal<Vec<SentimentData>>,
) -> impl IntoView {
    let video_ref = NodeRef::<leptos::html::Video>::new();
    
    // 웹캠 시작
    Effect::new(move |_| {
        if is_active.get() {
            let video = video_ref.get();
            if let Some(video_el) = video {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = start_camera(&video_el).await {
                        log::error!("Camera error: {:?}", e);
                    }
                });
            }
        }
    });
    
    // 프레임 캡처 및 분석
    Effect::new(move |_| {
        if is_active.get() {
            let case = case_id.get();
            if let Some(cid) = case {
                let handle = gloo_timers::callback::Interval::new(3000, move || {
                    let cid = cid.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        log::debug!("Analyzing frame: {}", cid);
                    });
                });
                handle.forget();
            }
        }
    });
    
    view! {
        <div class="grok-card rounded-md p-4">
            <div class="flex items-center justify-between mb-3">
                <h2 class="text-xs font-medium text-grok-muted uppercase tracking-wider flex items-center gap-2">
                    {icons::video()}
                    "Client Feed"
                </h2>
                <Show when=move || is_active.get()>
                    <div class="flex items-center gap-1.5 text-red-500">
                        <div class="w-1.5 h-1.5 rounded-full bg-red-500 animate-pulse" />
                        <span class="text-xs font-mono">"LIVE"</span>
                    </div>
                </Show>
            </div>
            
            <div class="relative aspect-video bg-grok-black rounded border border-grok-border overflow-hidden">
                <video
                    node_ref=video_ref
                    autoplay=true
                    playsinline=true
                    muted=true
                    class="w-full h-full object-cover"
                />
                
                // 대기 화면
                <Show when=move || !is_active.get()>
                    <div class="absolute inset-0 flex flex-col items-center justify-center bg-grok-black">
                        <div class="text-grok-muted mb-2">
                            {icons::scan()}
                        </div>
                        <p class="text-xs text-grok-muted font-mono">"Start case to begin"</p>
                    </div>
                </Show>
                
                // 분석 오버레이
                <Show when=move || is_active.get()>
                    <div class="absolute bottom-0 left-0 right-0 p-2 bg-gradient-to-t from-black/80 to-transparent">
                        <div class="flex items-center justify-between text-xs">
                            <span class="text-grok-muted font-mono">"Scanning..."</span>
                            <div class="flex items-center gap-1">
                                <div class="w-1 h-1 rounded-full bg-grok-blue animate-ping" />
                                <span class="text-grok-blue font-mono">"AI Active"</span>
                            </div>
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
}

async fn start_camera(video: &HtmlVideoElement) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let navigator = window.navigator();
    let media_devices = navigator.media_devices()?;
    
    let mut constraints = MediaStreamConstraints::new();
    constraints.set_video(&JsValue::TRUE);
    constraints.set_audio(&JsValue::TRUE);
    
    let stream_promise = media_devices.get_user_media_with_constraints(&constraints)?;
    let stream = JsFuture::from(stream_promise).await?;
    let media_stream = web_sys::MediaStream::from(stream);
    
    video.set_src_object(Some(&media_stream));
    
    Ok(())
}
