use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{AudioContext, AnalyserNode, MediaStream, HtmlCanvasElement, CanvasRenderingContext2d, WebSocket, MessageEvent};
use std::cell::RefCell;
use std::rc::Rc;

/// 음성 시각화 컴포넌트 (Time Domain + FFT 스펙트럼 + STT 텍스트 + VAD)
#[component]
pub fn AudioVisualizer(
    #[prop(into)] is_active: Signal<bool>,
    #[prop(into, optional)] transcript: Option<Signal<String>>,
    #[prop(into, optional)] on_speech_end: Option<Callback<()>>,
    #[prop(default = 260)] width: u32,
    #[prop(default = 100)] height: u32,
) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let (vad_status, set_vad_status) = signal(String::from("대기"));
    let (speech_prob, set_speech_prob) = signal(0.0f32);
    
    // 오디오 컨텍스트와 분석기 상태
    let audio_state: Rc<RefCell<Option<AudioState>>> = Rc::new(RefCell::new(None));
    
    // 활성화 상태 변경 시 오디오 처리
    Effect::new({
        let audio_state = audio_state.clone();
        let canvas_ref = canvas_ref.clone();
        move |_| {
            let active = is_active.get();
            
            if active {
                // 이전 오디오 리소스 정리 (재시작 시 누수 방지)
                if let Some(old_state) = audio_state.borrow_mut().take() {
                    let _ = old_state.audio_context.close();
                    if let Some(ws) = &old_state.websocket {
                        let _ = ws.close();
                    }
                    log::info!("Cleaned up previous audio state before restart");
                }

                // 마이크 시작 + 시각화 + VAD
                let audio_state = audio_state.clone();
                let canvas_ref = canvas_ref.clone();

                spawn_local(async move {
                    if let Some(state) = start_audio_capture_with_vad(
                        set_vad_status,
                        set_speech_prob,
                        on_speech_end,
                    ).await {
                        *audio_state.borrow_mut() = Some(state.clone());

                        if let Some(canvas) = canvas_ref.get() {
                            start_visualization(canvas, state, speech_prob);
                        }
                    }
                });
            } else {
                // 정리
                if let Some(state) = audio_state.borrow_mut().take() {
                    let _ = state.audio_context.close();
                    if let Some(ws) = &state.websocket {
                        let _ = ws.close();
                    }
                }
                set_vad_status.set("대기".to_string());
                set_speech_prob.set(0.0);
            }
        }
    });

    view! {
        <div class="flex flex-col gap-2">
            // 캔버스 (Time Domain 위, FFT 아래)
            <canvas
                node_ref=canvas_ref
                width=width
                height=height
                class="rounded-lg bg-gray-800/50 w-full"
            />
            
            // VAD 상태 표시
            <div class="flex items-center gap-2 text-xs">
                <span class="text-grok-muted">"VAD:"</span>
                <span class=move || {
                    let prob = speech_prob.get();
                    if prob > 0.5 { "text-green-400 font-bold" } else { "text-gray-500" }
                }>
                    {move || vad_status.get()}
                </span>
                <div class="flex-1 bg-gray-700 rounded-full h-1.5">
                    <div 
                        class="bg-green-500 h-1.5 rounded-full transition-all duration-100"
                        style=move || format!("width: {}%", (speech_prob.get() * 100.0) as u32)
                    />
                </div>
            </div>
            
            // STT 실시간 텍스트
            {move || transcript.map(|t| {
                let text = t.get();
                if !text.is_empty() {
                    view! {
                        <div class="text-sm text-gray-300 bg-gray-800/70 rounded-lg px-3 py-2 min-h-[2rem] flex items-center">
                            <span class="text-orange-400 mr-2">"🗣️"</span>
                            <span class="italic">{text}</span>
                            <span class="animate-pulse ml-1">"▊"</span>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="text-sm text-gray-500 bg-gray-800/50 rounded-lg px-3 py-2 min-h-[2rem] flex items-center">
                            <span class="text-gray-600 mr-2">"🎤"</span>
                            <span>"말해봐..."</span>
                        </div>
                    }.into_any()
                }
            })}
        </div>
    }
}

#[derive(Clone)]
struct AudioState {
    audio_context: AudioContext,
    analyser: AnalyserNode,
    #[allow(dead_code)]
    stream: MediaStream,
    websocket: Option<WebSocket>,
}

async fn start_audio_capture_with_vad(
    set_vad_status: WriteSignal<String>,
    set_speech_prob: WriteSignal<f32>,
    on_speech_end: Option<Callback<()>>,
) -> Option<AudioState> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen_futures::JsFuture;
        
        let window = web_sys::window()?;
        let navigator = window.navigator();
        let media_devices = navigator.media_devices().ok()?;
        
        // 마이크 권한 요청 (노이즈 제거 활성화)
        let constraints = web_sys::MediaStreamConstraints::new();
        
        // 오디오 설정: 노이즈/에코 제거 활성화
        let audio_constraints = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&audio_constraints, &"noiseSuppression".into(), &JsValue::TRUE);
        let _ = js_sys::Reflect::set(&audio_constraints, &"echoCancellation".into(), &JsValue::TRUE);
        let _ = js_sys::Reflect::set(&audio_constraints, &"autoGainControl".into(), &JsValue::TRUE);
        let _ = js_sys::Reflect::set(&audio_constraints, &"sampleRate".into(), &JsValue::from(16000));
        
        constraints.set_audio(&audio_constraints.into());
        constraints.set_video(&JsValue::FALSE);
        
        let stream_promise = media_devices.get_user_media_with_constraints(&constraints).ok()?;
        let stream: MediaStream = JsFuture::from(stream_promise).await.ok()?.dyn_into().ok()?;
        
        // AudioContext 생성
        let audio_context = AudioContext::new().ok()?;
        
        // AnalyserNode 설정
        let analyser = audio_context.create_analyser().ok()?;
        analyser.set_fft_size(512);
        analyser.set_smoothing_time_constant(0.85);
        
        // 마이크 → 분석기 연결
        let source = audio_context.create_media_stream_source(&stream).ok()?;
        source.connect_with_audio_node(&analyser).ok()?;
        
        // VAD WebSocket 연결
        let ws = connect_vad_websocket(set_vad_status, set_speech_prob, on_speech_end);
        
        // ScriptProcessor로 오디오 데이터 추출 및 VAD로 전송
        if let (Some(ws_ref), Ok(processor)) = (&ws, audio_context.create_script_processor_with_buffer_size(512)) {
            let ws_clone = ws_ref.clone();
            
            let onaudioprocess = Closure::wrap(Box::new(move |event: web_sys::AudioProcessingEvent| {
                if let Ok(input_buffer) = event.input_buffer() {
                    if let Ok(channel_data) = input_buffer.get_channel_data(0) {
                        // Float32 PCM 데이터를 WebSocket으로 전송
                        let bytes: Vec<u8> = channel_data.iter()
                            .flat_map(|f| f.to_le_bytes())
                            .collect();
                        
                        if ws_clone.ready_state() == WebSocket::OPEN {
                            let _ = ws_clone.send_with_u8_array(&bytes);
                        }
                    }
                }
            }) as Box<dyn FnMut(web_sys::AudioProcessingEvent)>);
            
            processor.set_onaudioprocess(Some(onaudioprocess.as_ref().unchecked_ref()));
            onaudioprocess.forget();
            
            // 연결: source → processor → destination (muted)
            let _ = source.connect_with_audio_node(&processor);
            let _ = processor.connect_with_audio_node(&audio_context.destination());
        }
        
        Some(AudioState {
            audio_context,
            analyser,
            stream,
            websocket: ws,
        })
    }
    
    #[cfg(not(target_arch = "wasm32"))]
    None
}

fn connect_vad_websocket(
    set_vad_status: WriteSignal<String>,
    set_speech_prob: WriteSignal<f32>,
    on_speech_end: Option<Callback<()>>,
) -> Option<WebSocket> {
    #[cfg(target_arch = "wasm32")]
    {
        // Origin-relative WebSocket URL — page protocol(http/https)에 따라 ws/wss 자동 선택.
        // dev: Trunk proxy(/vad-ws → ws://127.0.0.1:8085), prod: cloudflared ingress(/vad-ws → :8085).
        let ws_url = {
            let location = web_sys::window()?.location();
            let proto = if location.protocol().ok()? == "https:" { "wss:" } else { "ws:" };
            let host = location.host().ok()?;
            format!("{}//{}/vad-ws", proto, host)
        };
        log::info!("Connecting VAD WebSocket: {}", ws_url);
        let ws = WebSocket::new(&ws_url).ok()?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);
        
        // 메시지 수신
        let set_status = set_vad_status.clone();
        let set_prob = set_speech_prob.clone();
        
        let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
            if let Ok(text) = event.data().dyn_into::<js_sys::JsString>() {
                let text: String = text.into();
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    let event_type = json["event"].as_str().unwrap_or("");
                    
                    match event_type {
                        "speech_start" => {
                            set_status.set("🗣️ 말하는 중".to_string());
                        }
                        "speech_end" => {
                            set_status.set("🔇 종료".to_string());
                            if let Some(callback) = &on_speech_end {
                                callback.run(());
                            }
                        }
                        "vad_probability" => {
                            if let Some(prob) = json["probability"].as_f64() {
                                set_prob.set(prob as f32);
                            }
                            if json["is_speaking"].as_bool().unwrap_or(false) {
                                set_status.set("🗣️ 말하는 중".to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();
        
        // 연결 성공
        let set_status_open = set_vad_status.clone();
        let onopen = Closure::wrap(Box::new(move |_: web_sys::Event| {
            set_status_open.set("✅ VAD 연결됨".to_string());
            log::info!("VAD WebSocket connected");
        }) as Box<dyn FnMut(web_sys::Event)>);
        
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget();
        
        // 연결 실패
        let set_status_err = set_vad_status.clone();
        let onerror = Closure::wrap(Box::new(move |_: web_sys::Event| {
            set_status_err.set("❌ VAD 연결 실패".to_string());
            log::error!("VAD WebSocket error");
        }) as Box<dyn FnMut(web_sys::Event)>);
        
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
        
        Some(ws)
    }
    
    #[cfg(not(target_arch = "wasm32"))]
    None
}

fn start_visualization(canvas: HtmlCanvasElement, state: AudioState, speech_prob: ReadSignal<f32>) {
    #[cfg(target_arch = "wasm32")]
    {
        let ctx: CanvasRenderingContext2d = match canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|c| c.dyn_into().ok())
        {
            Some(ctx) => ctx,
            None => {
                log::warn!("Canvas context unavailable, skipping visualization");
                return;
            }
        };
        
        let width = canvas.width() as f64;
        let height = canvas.height() as f64;
        let half_height = height / 2.0;
        let analyser = state.analyser.clone();
        let fft_size = analyser.fft_size() as usize;
        let buffer_length = analyser.frequency_bin_count() as usize;
        
        // 애니메이션 루프
        let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
        let g = f.clone();
        
        *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
            // 시간 도메인 데이터 (파형)
            let mut time_data = vec![0u8; fft_size];
            analyser.get_byte_time_domain_data(&mut time_data);
            
            // 주파수 도메인 데이터 (FFT)
            let mut freq_data = vec![0u8; buffer_length];
            analyser.get_byte_frequency_data(&mut freq_data);
            
            // 캔버스 클리어
            ctx.set_fill_style_str("#1f2937");
            ctx.fill_rect(0.0, 0.0, width, height);
            
            // === Time Domain (파형) - 상단 절반 ===
            let prob = speech_prob.get();
            let wave_color = if prob > 0.5 { "#22c55e" } else { "#f97316" };  // green or orange
            ctx.set_stroke_style_str(wave_color);
            ctx.set_line_width(1.5);
            ctx.begin_path();
            
            let slice_width = width / time_data.len() as f64;
            let mut x = 0.0;
            
            for (i, &sample) in time_data.iter().enumerate() {
                let v = sample as f64 / 128.0;
                let y = (v * half_height / 2.0) + (half_height / 2.0);
                
                if i == 0 {
                    ctx.move_to(x, y);
                } else {
                    ctx.line_to(x, y);
                }
                x += slice_width;
            }
            
            ctx.stroke();
            
            // 중간 구분선
            ctx.set_stroke_style_str("#374151");
            ctx.begin_path();
            ctx.move_to(0.0, half_height);
            ctx.line_to(width, half_height);
            ctx.stroke();
            
            // === FFT 스펙트럼 - 하단 절반 ===
            let bar_count = 64;
            let bar_width = width / bar_count as f64;
            let step = buffer_length / bar_count;
            
            for i in 0..bar_count {
                let mut sum = 0u32;
                for j in 0..step {
                    sum += freq_data[i * step + j] as u32;
                }
                let value = (sum / step as u32) as f64 / 255.0;
                
                let bar_height = value * (half_height - 4.0);
                let x = i as f64 * bar_width;
                let y = height - bar_height;
                
                let hue = if prob > 0.5 { 120.0 } else { 35.0 - (value * 35.0) };
                let saturation = 85.0 + (value * 15.0);
                let lightness = 50.0 + (value * 15.0);
                
                ctx.set_fill_style_str(&format!("hsl({}, {}%, {}%)", hue, saturation, lightness));
                ctx.fill_rect(x, y, bar_width - 1.0, bar_height);
            }
            
            // 라벨
            ctx.set_fill_style_str("#9ca3af");
            ctx.set_font("9px monospace");
            let _ = ctx.fill_text("Waveform", 4.0, 12.0);
            let _ = ctx.fill_text("Spectrum", 4.0, half_height + 12.0);
            
            // 다음 프레임 요청
            if let Some(window) = web_sys::window() {
                let _ = window.request_animation_frame(
                    f.borrow().as_ref().unwrap().as_ref().unchecked_ref()
                );
            }
        }) as Box<dyn FnMut()>));
        
        // 첫 프레임 시작
        if let Some(window) = web_sys::window() {
            let _ = window.request_animation_frame(
                g.borrow().as_ref().unwrap().as_ref().unchecked_ref()
            );
        }
    }
}
