use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlVideoElement, HtmlCanvasElement, MediaStreamConstraints, CanvasRenderingContext2d};
use crate::app::SentimentData;
use crate::components::icons;
use crate::components::face_analysis::{ActionUnits, EmotionState, FaceAnalysisOverlay};
use crate::components::rppg::{RppgAnalyzer, HeartRateResult, TensionLevel};
use crate::components::vital_signs::HeartRateMini;
use std::cell::RefCell;
use std::rc::Rc;

// JavaScript 함수 바인딩
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window, catch)]
    async fn initFaceMesh() -> Result<(), JsValue>;
    
    #[wasm_bindgen(js_namespace = window, catch)]
    async fn detectFace(video: &HtmlVideoElement) -> Result<JsValue, JsValue>;
    
    #[wasm_bindgen(js_namespace = window)]
    fn computeAUs(landmarks: JsValue) -> JsValue;
    
    #[wasm_bindgen(js_namespace = window)]
    fn inferEmotion(aus: JsValue) -> JsValue;
}

#[component]
pub fn VideoCapture(
    is_active: ReadSignal<bool>,
    #[allow(unused)]
    case_id: ReadSignal<Option<String>>,
    #[allow(unused)]
    set_sentiments: WriteSignal<Vec<SentimentData>>,
    video_enabled: ReadSignal<bool>,
    set_video_enabled: WriteSignal<bool>,
) -> impl IntoView {
    let video_ref = NodeRef::<leptos::html::Video>::new();
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    // 카메라 스트림 핸들 (토글 OFF 시 트랙 정지용)
    let media_stream: Rc<RefCell<Option<web_sys::MediaStream>>> = Rc::new(RefCell::new(None));
    
    // 감정 분석 상태
    let (emotion, set_emotion) = signal(EmotionState::default());
    let (action_units, set_action_units) = signal(ActionUnits::default());
    let (dominant, set_dominant) = signal("😐 대기중");
    let (is_analyzing, set_is_analyzing) = signal(false);
    let (face_detected, set_face_detected) = signal(false);
    
    // rPPG 심박수 상태
    let (heart_rate, set_heart_rate) = signal(HeartRateResult::default());
    let (bpm_display, set_bpm_display) = signal(0.0_f64);
    let (confidence_display, set_confidence_display) = signal(0.0_f64);
    let (tension_level, set_tension_level) = signal(TensionLevel::Normal);
    
    // rPPG 분석기 (Rc<RefCell>로 공유)
    let rppg_analyzer: Rc<RefCell<RppgAnalyzer>> = Rc::new(RefCell::new(RppgAnalyzer::new()));
    
    // MediaPipe 초기화 (영상 분석 활성화 시에만)
    Effect::new(move |_| {
        if is_active.get() && video_enabled.get() {
            wasm_bindgen_futures::spawn_local(async move {
                match initFaceMesh().await {
                    Ok(_) => log::info!("🦊 MediaPipe Face Mesh initialized"),
                    Err(e) => log::error!("MediaPipe init error: {:?}", e),
                }
            });
        }
    });

    // 웹캠 시작/정지 (is_active && video_enabled 상태에 따라)
    {
        let media_stream = media_stream.clone();
        Effect::new(move |_| {
            let should_run = is_active.get() && video_enabled.get();
            let video = video_ref.get();

            if should_run {
                if let Some(video_el) = video {
                    let media_stream = media_stream.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        match start_camera(&video_el).await {
                            Ok(stream) => {
                                *media_stream.borrow_mut() = Some(stream);
                            }
                            Err(e) => log::error!("Camera error: {:?}", e),
                        }
                    });
                }
            } else {
                // 카메라 트랙 정지
                if let Some(stream) = media_stream.borrow_mut().take() {
                    let tracks = stream.get_tracks();
                    for i in 0..tracks.length() {
                        if let Ok(track) = tracks.get(i).dyn_into::<web_sys::MediaStreamTrack>() {
                            track.stop();
                        }
                    }
                }
                if let Some(video_el) = video {
                    video_el.set_src_object(None);
                }
                // 분석 상태 초기화
                set_face_detected.set(false);
                set_bpm_display.set(0.0);
                set_confidence_display.set(0.0);
                set_dominant.set("😐 대기중");
            }
        });
    }
    
    // 얼굴 분석 + rPPG 루프
    {
        let rppg = rppg_analyzer.clone();
        Effect::new(move |_| {
            if is_active.get() && video_enabled.get() && !is_analyzing.get() {
                set_is_analyzing.set(true);
                let rppg = rppg.clone();

                let handle = gloo_timers::callback::Interval::new(500, move || {
                    // 런타임에 토글이 꺼지거나 세션이 종료되면 스킵
                    if !is_active.get() || !video_enabled.get() {
                        return;
                    }

                    let video_el = video_ref.get();
                    let canvas_el = canvas_ref.get();

                    if let (Some(video), Some(canvas)) = (video_el, canvas_el) {
                        let video = video.clone();
                        let canvas = canvas.clone();
                        let rppg = rppg.clone();
                        
                        wasm_bindgen_futures::spawn_local(async move {
                            // 1. 얼굴 감지
                            match detectFace(&video).await {
                                Ok(result) => {
                                    if !result.is_null() && !result.is_undefined() {
                                        set_face_detected.set(true);
                                        
                                        // landmarks 추출
                                        if let Ok(landmarks) = js_sys::Reflect::get(&result, &"landmarks".into()) {
                                            // AU 계산 (JS에서)
                                            let aus = computeAUs(landmarks.clone());
                                            if !aus.is_null() && !aus.is_undefined() {
                                                // 감정 추론 (JS에서)
                                                let emo = inferEmotion(aus.clone());
                                                
                                                // Rust 구조체로 변환
                                                if let Ok(emotion_state) = serde_wasm_bindgen::from_value::<JsEmotionState>(emo) {
                                                    let rust_emotion = EmotionState {
                                                        happiness: emotion_state.happiness,
                                                        sadness: emotion_state.sadness,
                                                        anger: emotion_state.anger,
                                                        fear: 0.0,
                                                        surprise: emotion_state.surprise,
                                                        disgust: emotion_state.disgust,
                                                        contempt: 0.0,
                                                        neutral: emotion_state.neutral,
                                                        valence: emotion_state.valence,
                                                        arousal: emotion_state.arousal,
                                                        dominance: emotion_state.dominance,
                                                    };
                                                    
                                                    set_dominant.set(match emotion_state.dominant.as_str() {
                                                        "happiness" => "😊 행복",
                                                        "sadness" => "😢 슬픔",
                                                        "anger" => "😠 분노",
                                                        "surprise" => "😲 놀람",
                                                        "disgust" => "🤢 혐오",
                                                        _ => "😐 중립",
                                                    });
                                                    
                                                    set_emotion.set(rust_emotion);
                                                }
                                                
                                                // AU도 변환
                                                if let Ok(au_state) = serde_wasm_bindgen::from_value::<JsActionUnits>(aus) {
                                                    let rust_au = ActionUnits {
                                                        au1_inner_brow_raiser: au_state.au1,
                                                        au2_outer_brow_raiser: au_state.au2,
                                                        au4_brow_lowerer: au_state.au4,
                                                        au5_upper_lid_raiser: au_state.au5,
                                                        au6_cheek_raiser: au_state.au6,
                                                        au12_lip_corner_puller: au_state.au12,
                                                        au15_lip_corner_depressor: au_state.au15,
                                                        au25_lips_part: au_state.au25,
                                                        au26_jaw_drop: au_state.au26,
                                                        ..Default::default()
                                                    };
                                                    set_action_units.set(rust_au);
                                                }
                                            }
                                            
                                            // 2. rPPG: 이마/볼 영역 Green 채널 추출
                                            if let Ok(landmarks_arr) = serde_wasm_bindgen::from_value::<Vec<[f64; 3]>>(landmarks) {
                                                if let Some(green_mean) = extract_roi_green(&video, &canvas, &landmarks_arr) {
                                                    let timestamp = js_sys::Date::now() / 1000.0;
                                                    
                                                    // rPPG 분석기에 샘플 추가
                                                    let mut analyzer = rppg.borrow_mut();
                                                    analyzer.add_sample(green_mean, timestamp);
                                                    
                                                    // 분석 실행
                                                    let hr_result = analyzer.analyze();
                                                    let tension = TensionLevel::from_result(&hr_result);
                                                    
                                                    set_bpm_display.set(hr_result.bpm);
                                                    set_confidence_display.set(hr_result.confidence);
                                                    set_tension_level.set(tension);
                                                    set_heart_rate.set(hr_result);
                                                }
                                            }
                                        }
                                    } else {
                                        set_face_detected.set(false);
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Face detection error: {:?}", e);
                                    set_face_detected.set(false);
                                }
                            }
                        });
                    }
                });
                handle.forget();
            }
        });
    }
    
    view! {
        <div class="grok-card rounded-md p-4 relative">
            <div class="flex items-center justify-between mb-3">
                <h2 class="text-xs font-medium text-grok-muted uppercase tracking-wider flex items-center gap-2">
                    {icons::video()}
                    "Client Feed"
                </h2>
                <div class="flex items-center gap-3">
                    // 영상 분석 ON/OFF 토글
                    <button
                        type="button"
                        on:click=move |_| set_video_enabled.update(|v| *v = !*v)
                        class=move || {
                            let base = "flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-mono uppercase tracking-wider border transition-colors";
                            if video_enabled.get() {
                                format!("{} bg-grok-blue/20 border-grok-blue/40 text-grok-blue hover:bg-grok-blue/30", base)
                            } else {
                                format!("{} bg-grok-gray border-grok-border text-grok-muted hover:bg-grok-border", base)
                            }
                        }
                        title="영상 분석 On/Off"
                    >
                        <div class=move || {
                            if video_enabled.get() {
                                "w-1.5 h-1.5 rounded-full bg-grok-blue"
                            } else {
                                "w-1.5 h-1.5 rounded-full bg-grok-muted"
                            }
                        } />
                        {move || if video_enabled.get() { "Video On" } else { "Video Off" }}
                    </button>
                    // 얼굴 감지 상태
                    <Show when=move || face_detected.get() && video_enabled.get()>
                        <div class="flex items-center gap-1.5 text-green-500">
                            <div class="w-1.5 h-1.5 rounded-full bg-green-500" />
                            <span class="text-xs font-mono">"Face"</span>
                        </div>
                    </Show>
                    // 긴장도 이모지
                    <Show when=move || is_active.get() && face_detected.get() && video_enabled.get()>
                        <span class="text-sm" title="긴장도">
                            {move || tension_level.get().emoji()}
                        </span>
                    </Show>
                    // 지배적 감정
                    <Show when=move || is_active.get() && video_enabled.get()>
                        <span class="text-xs font-mono text-grok-blue">
                            {move || dominant.get()}
                        </span>
                    </Show>
                    // LIVE 표시
                    <Show when=move || is_active.get() && video_enabled.get()>
                        <div class="flex items-center gap-1.5 text-red-500">
                            <div class="w-1.5 h-1.5 rounded-full bg-red-500 animate-pulse" />
                            <span class="text-xs font-mono">"LIVE"</span>
                        </div>
                    </Show>
                </div>
            </div>
            
            <div class="relative aspect-video bg-grok-black rounded border border-grok-border overflow-hidden">
                <video
                    node_ref=video_ref
                    autoplay=true
                    playsinline=true
                    muted=true
                    class="w-full h-full object-cover"
                />
                
                // 숨겨진 캔버스 (ROI 추출용)
                <canvas
                    node_ref=canvas_ref
                    class="hidden"
                    width="640"
                    height="480"
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

                // 영상 분석 OFF 플레이스홀더 (세션 진행 중이지만 토글 꺼짐)
                <Show when=move || is_active.get() && !video_enabled.get()>
                    <div class="absolute inset-0 flex flex-col items-center justify-center bg-grok-black/95">
                        <div class="text-grok-muted mb-2">
                            {icons::video()}
                        </div>
                        <p class="text-sm text-white font-medium mb-1">"Video Analysis Off"</p>
                        <p class="text-xs text-grok-muted font-mono">"음성 분석만 진행 중"</p>
                    </div>
                </Show>

                // 얼굴 분석 오버레이 (왼쪽 상단)
                <Show when=move || is_active.get() && video_enabled.get() && face_detected.get()>
                    <FaceAnalysisOverlay
                        emotion=emotion
                        action_units=action_units
                        show_details=false
                    />
                </Show>

                // 심박수 오버레이 (오른쪽 상단)
                <Show when=move || is_active.get() && video_enabled.get() && face_detected.get()>
                    <div class="absolute top-2 right-2">
                        <HeartRateMini
                            bpm=bpm_display
                            confidence=confidence_display
                        />
                    </div>
                </Show>

                // 분석 오버레이 (하단)
                <Show when=move || is_active.get() && video_enabled.get()>
                    <div class="absolute bottom-0 left-0 right-0 p-2 bg-gradient-to-t from-black/80 to-transparent">
                        <div class="flex items-center justify-between text-xs">
                            <span class="text-grok-muted font-mono">
                                {move || if face_detected.get() {
                                    format!("HR: {:.0} BPM | {}", bpm_display.get(), tension_level.get().label())
                                } else {
                                    "Scanning...".to_string()
                                }}
                            </span>
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

/// 카메라 시작
async fn start_camera(video: &HtmlVideoElement) -> Result<web_sys::MediaStream, JsValue> {
    let window = web_sys::window().unwrap();
    let navigator = window.navigator();
    let media_devices = navigator.media_devices()?;

    let constraints = MediaStreamConstraints::new();
    constraints.set_video(&JsValue::TRUE);
    constraints.set_audio(&JsValue::FALSE);

    let stream_promise = media_devices.get_user_media_with_constraints(&constraints)?;
    let stream = JsFuture::from(stream_promise).await?;
    let media_stream = web_sys::MediaStream::from(stream);

    video.set_src_object(Some(&media_stream));

    Ok(media_stream)
}

/// ROI에서 Green 채널 평균 추출 (이마 + 양 볼)
fn extract_roi_green(
    video: &HtmlVideoElement, 
    canvas: &HtmlCanvasElement,
    landmarks: &[[f64; 3]]
) -> Option<f64> {
    if landmarks.len() < 468 {
        return None;
    }
    
    let ctx = canvas.get_context("2d").ok()??.dyn_into::<CanvasRenderingContext2d>().ok()?;
    
    let video_width = video.video_width() as f64;
    let video_height = video.video_height() as f64;
    
    if video_width == 0.0 || video_height == 0.0 {
        return None;
    }
    
    // 비디오를 캔버스에 그리기
    canvas.set_width(video_width as u32);
    canvas.set_height(video_height as u32);
    ctx.draw_image_with_html_video_element(video, 0.0, 0.0).ok()?;
    
    // ROI 포인트 (이마: 10, 67, 69, 104, 108 / 왼쪽볼: 116, 117, 118 / 오른쪽볼: 345, 346, 347)
    let roi_indices = [10, 67, 69, 104, 108, 116, 117, 118, 345, 346, 347, 234, 454];
    
    let mut green_sum = 0.0;
    let mut pixel_count = 0;
    
    for &idx in &roi_indices {
        if idx >= landmarks.len() {
            continue;
        }
        
        let x = (landmarks[idx][0] * video_width) as i32;
        let y = (landmarks[idx][1] * video_height) as i32;
        
        // 각 포인트 주변 5x5 영역 샘플링
        for dx in -2..=2 {
            for dy in -2..=2 {
                let px = (x + dx).max(0).min(video_width as i32 - 1);
                let py = (y + dy).max(0).min(video_height as i32 - 1);
                
                if let Ok(image_data) = ctx.get_image_data(px as f64, py as f64, 1.0, 1.0) {
                    let data = image_data.data();
                    if data.len() >= 4 {
                        green_sum += data[1] as f64; // Green 채널
                        pixel_count += 1;
                    }
                }
            }
        }
    }
    
    if pixel_count > 0 {
        Some(green_sum / pixel_count as f64)
    } else {
        None
    }
}

// JS 데이터 구조체
#[derive(serde::Deserialize)]
struct JsEmotionState {
    happiness: f32,
    sadness: f32,
    anger: f32,
    surprise: f32,
    disgust: f32,
    neutral: f32,
    dominant: String,
    valence: f32,
    arousal: f32,
    dominance: f32,
}

#[derive(serde::Deserialize)]
struct JsActionUnits {
    au1: f32,
    au2: f32,
    au4: f32,
    au5: f32,
    au6: f32,
    au12: f32,
    au15: f32,
    au25: f32,
    au26: f32,
}
