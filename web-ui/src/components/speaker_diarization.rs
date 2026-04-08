use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{MediaStream, AudioContext, MediaStreamAudioSourceNode, AudioBuffer};
use std::rc::Rc;
use std::cell::RefCell;
use crate::components::resemblyzer::ResemblyzerEngine;

/// 화자 분리 관리 컴포넌트 (Resemblyzer ONNX 기반)
#[component]
pub fn SpeakerDiarization(
    #[prop(optional)] audio_stream: Option<ReadSignal<Option<web_sys::MediaStream>>>,
    is_active: ReadSignal<bool>,
    on_speaker_change: WriteSignal<Option<String>>,
) -> impl IntoView {
    let (current_speaker, set_current_speaker) = signal(None::<String>);
    let (status_text, set_status_text) = signal("Idle".to_string());
    let (speaker_count, set_speaker_count) = signal(0_usize);
    let (model_status, set_model_status) = signal("Loading...".to_string());
    
    // Resemblyzer 엔진 (ONNX 기반)
    let engine = Rc::new(RefCell::new(ResemblyzerEngine::new()));
    
    // 모델 상태 확인
    {
        let engine_ref = engine.clone();
        Effect::new(move |_| {
            let is_available = engine_ref.borrow().is_model_available();
            if is_available {
                set_model_status.set("✅ ONNX Model".to_string());
            } else {
                set_model_status.set("⚠️ Fallback Mode".to_string());
            }
        });
    }
    
    // 화자 변경 시 외부 signal 업데이트
    Effect::new(move |_| {
        let speaker = current_speaker.get();
        on_speaker_change.set(speaker);
    });
    
    // 활성 상태 변경 시 처리
    Effect::new(move |_| {
        let engine_clone = engine.clone();
        
        if is_active.get() {
            set_status_text.set("Listening...".to_string());
            
            if let Some(stream_signal) = audio_stream {
                spawn_local(async move {
                    start_audio_analysis(
                        stream_signal,
                        engine_clone,
                        set_current_speaker,
                        set_status_text,
                        set_speaker_count,
                    ).await;
                });
            } else {
                // 오디오 스트림이 없으면 대기
                spawn_local(async move {
                    wait_for_audio(
                        set_current_speaker,
                        set_status_text,
                        set_speaker_count,
                    ).await;
                });
            }
        } else {
            set_current_speaker.set(None);
            set_status_text.set("Idle".to_string());
            set_speaker_count.set(0);
            
            // 엔진 초기화
            engine_clone.borrow_mut().reset();
        }
    });
    
    view! {
        <div class="grok-card rounded-md p-3 mb-2">
            <h3 class="text-xs font-medium text-grok-muted uppercase tracking-wider mb-2 flex items-center justify-between">
                <span>"🎙️ Speaker Analysis (Resemblyzer)"</span>
                <span class="text-grok-blue font-mono text-[10px]">
                    {move || model_status.get()}
                </span>
            </h3>
            
            // 화자 수 표시
            <div class="flex items-center justify-between mb-2">
                <span class="text-xs text-grok-muted">"Detected speakers:"</span>
                <span class="text-sm text-white font-mono font-bold">
                    {move || speaker_count.get()}
                </span>
            </div>
            
            // 상태 표시
            <div class="flex items-center gap-2 mb-3">
                <div class={move || {
                    if is_active.get() {
                        "w-2 h-2 rounded-full bg-green-500 animate-pulse"
                    } else {
                        "w-2 h-2 rounded-full bg-gray-500"
                    }
                }}></div>
                <span class="text-xs text-grok-text font-mono">
                    {move || status_text.get()}
                </span>
            </div>
            
            // 현재 화자 표시
            <Show when=move || current_speaker.get().is_some()>
                <div class="flex items-center gap-2 p-2 bg-grok-gray rounded">
                    <div class="w-3 h-3 rounded-full bg-blue-500"></div>
                    <span class="text-sm text-white font-medium">
                        {move || current_speaker.get().unwrap_or_default()}
                    </span>
                    <div class="ml-auto text-xs text-grok-muted font-mono">
                        "Active"
                    </div>
                </div>
            </Show>
            
            // 기술 정보
            <div class="mt-2 pt-2 border-t border-grok-border">
                <div class="text-[10px] text-grok-muted font-mono space-y-1">
                    <div>"• Model: GE2E LSTM (resemblyzer)"</div>
                    <div>"• Embedding: 256-dim speaker vector"</div>
                    <div>"• Threshold: 0.75 cosine similarity"</div>
                </div>
            </div>
        </div>
    }
}

/// VAD (Voice Activity Detection) - 음성 에너지 임계값
const VAD_ENERGY_THRESHOLD: f32 = 0.001;

/// 음성 활동 감지
fn detect_voice_activity(audio_data: &[f32]) -> bool {
    if audio_data.is_empty() {
        return false;
    }
    let energy: f32 = audio_data.iter().map(|x| x * x).sum::<f32>() / audio_data.len() as f32;
    energy > VAD_ENERGY_THRESHOLD
}

/// 44.1kHz → 16kHz 다운샘플링
fn downsample_to_16k(audio: &[f32], source_rate: f32) -> Vec<f32> {
    let target_rate = 16000.0;
    let ratio = source_rate / target_rate;
    let target_len = (audio.len() as f32 / ratio) as usize;
    
    (0..target_len)
        .map(|i| {
            let src_idx = (i as f32 * ratio) as usize;
            audio.get(src_idx).copied().unwrap_or(0.0)
        })
        .collect()
}

/// 실제 오디오 스트림 분석
async fn start_audio_analysis(
    stream_signal: ReadSignal<Option<MediaStream>>,
    engine: Rc<RefCell<ResemblyzerEngine>>,
    set_current_speaker: WriteSignal<Option<String>>,
    set_status_text: WriteSignal<String>,
    set_speaker_count: WriteSignal<usize>,
) {
    match setup_audio_context(stream_signal).await {
        Ok(_) => {
            set_status_text.set("마이크 연결됨 - 음성 대기 중...".to_string());
            
            // 오디오 처리 루프 (2초마다)
            for _i in 0..120 { // 240초 (4분)
                gloo_timers::future::TimeoutFuture::new(2000).await;
                
                // TODO: 실제 Web Audio API에서 오디오 가져오기
                let audio_chunk = capture_audio_chunk();
                
                // VAD 체크
                if detect_voice_activity(&audio_chunk) {
                    // 16kHz로 다운샘플링
                    let audio_16k = downsample_to_16k(&audio_chunk, 44100.0);
                    
                    // Resemblyzer로 화자 식별
                    let speaker_id = {
                        let mut eng = engine.borrow_mut();
                        eng.identify_speaker(&audio_16k)
                    };
                    
                    if let Some(speaker) = speaker_id {
                        set_current_speaker.set(Some(speaker));
                        
                        let count = engine.borrow().get_speakers().len();
                        set_speaker_count.set(count);
                        
                        set_status_text.set(format!("🎙️ 음성 감지 ({} 화자)", count));
                    }
                } else {
                    set_status_text.set("🔇 음성 대기 중...".to_string());
                }
            }
            
            set_status_text.set("분석 완료".to_string());
        }
        Err(e) => {
            log::warn!("Audio setup failed: {}", e);
            wait_for_audio(set_current_speaker, set_status_text, set_speaker_count).await;
        }
    }
}

/// 오디오 컨텍스트 설정
async fn setup_audio_context(stream_signal: ReadSignal<Option<MediaStream>>) -> Result<(), String> {
    let stream = stream_signal.get()
        .ok_or("No audio stream available")?;

    let audio_context = AudioContext::new()
        .map_err(|e| format!("AudioContext creation failed: {:?}", e))?;

    let _source = audio_context.create_media_stream_source(&stream)
        .map_err(|e| format!("MediaStreamSource creation failed: {:?}", e))?;

    // TODO: ScriptProcessorNode 또는 AudioWorklet 연동
    
    Ok(())
}

/// 오디오 청크 캡처 (현재는 무음 반환)
fn capture_audio_chunk() -> Vec<f32> {
    // TODO: 실제 Web Audio API에서 버퍼 가져오기
    // 현재는 무음 반환 (VAD가 false 반환)
    vec![0.0; 88200] // 2초 at 44.1kHz
}

/// 오디오 대기
async fn wait_for_audio(
    set_current_speaker: WriteSignal<Option<String>>,
    set_status_text: WriteSignal<String>,
    set_speaker_count: WriteSignal<usize>,
) {
    set_status_text.set("대기 중 (마이크 접근 필요)".to_string());
    set_current_speaker.set(None);
    set_speaker_count.set(0);
}
