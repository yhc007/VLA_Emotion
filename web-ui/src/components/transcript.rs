use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Blob, BlobEvent, FormData, MediaRecorder, MediaRecorderOptions, MediaStream};
use send_wrapper::SendWrapper;
use std::cell::RefCell;
use std::rc::Rc;
use crate::app::{TranscriptEntry, SentimentData};
use crate::components::icons;
use crate::components::audio_visualizer::AudioVisualizer;
use crate::storage;

/// 백엔드 STT 엔드포인트.
const STT_ENDPOINT: &str = "http://localhost:8080/api/stt";

/// MediaRecorder 선호 MIME 타입 (WebM/Opus는 Whisper가 직접 지원).
const RECORDER_MIME: &str = "audio/webm;codecs=opus";

/// 녹음기 세션 상태 — speech_end 콜백과 MediaRecorder 사이에서 공유.
#[derive(Clone, Default)]
struct RecorderSlot {
    recorder: Rc<RefCell<Option<MediaRecorder>>>,
    stream: Rc<RefCell<Option<MediaStream>>>,
    /// 현재 녹음 세그먼트의 Blob 청크들.
    chunks: Rc<RefCell<Vec<Blob>>>,
}

/// Whisper STT 응답 — 백엔드 `SttResponse`와 동일 스키마.
#[derive(Clone, Debug, serde::Deserialize)]
struct SttResponse {
    text: String,
    #[serde(default)]
    #[allow(dead_code)]
    language: String,
    #[serde(default)]
    #[allow(dead_code)]
    duration_secs: f32,
    #[serde(default)]
    #[allow(dead_code)]
    confidence: f32,
}

#[component]
pub fn Transcript(
    is_active: ReadSignal<bool>,
    case_id: ReadSignal<Option<String>>,
    transcripts: ReadSignal<Vec<TranscriptEntry>>,
    set_transcripts: WriteSignal<Vec<TranscriptEntry>>,
    set_sentiments: WriteSignal<Vec<SentimentData>>,
    #[prop(optional)] set_reasoning_result: Option<WriteSignal<Option<ReasoningResult>>>,
    #[prop(optional)] current_speaker: Option<ReadSignal<Option<String>>>,
) -> impl IntoView {
    // 실시간 interim 텍스트
    let (interim_text, set_interim_text) = signal(String::new());
    // 마지막 분석 결과
    let (last_analysis, set_last_analysis) = signal(Option::<String>::None);
    // 마지막 추론 결과 (내부 + 외부 연동)
    let (last_reasoning, set_last_reasoning) = signal(Option::<ReasoningResult>::None);
    
    // 외부 signal 연동
    Effect::new(move |_| {
        if let Some(set_external) = set_reasoning_result {
            if let Some(r) = last_reasoning.get() {
                set_external.set(Some(r));
            }
        }
    });
    
    // 녹음기 + VAD 연동 상태 (speech_end 콜백이 자신의 MediaRecorder를 참조해야 하므로 공유)
    let recorder_slot = RecorderSlot::default();

    // 세션 활성/종료 — 마이크 열기, MediaRecorder 시작, 저장소 세션 등록
    {
        let recorder_slot = recorder_slot.clone();
        Effect::new(move |_| {
            if is_active.get() {
                if let Some(cid) = case_id.get() {
                    log::info!("Starting Whisper STT pipeline for case: {}", cid);

                    // 저장소 세션 등록
                    let session_id = cid.clone();
                    spawn_local(async move {
                        if let Some(storage) = storage::get_storage() {
                            if let Err(e) = storage.start_session(&session_id).await {
                                log::error!("Failed to start session {}: {}", session_id, e);
                            }
                        }
                    });

                    // 마이크 → MediaRecorder 시작 (WebM/Opus)
                    let slot = recorder_slot.clone();
                    spawn_local(async move {
                        if let Err(e) = start_recorder(&slot).await {
                            log::error!("Failed to start recorder: {}", e);
                        }
                    });
                }
            } else {
                set_interim_text.set(String::new());
                set_last_analysis.set(None);
                set_last_reasoning.set(None);

                // MediaRecorder 및 MediaStream 정리
                stop_recorder(&recorder_slot);

                // 저장소 세션 종료
                if let Some(cid) = case_id.get() {
                    let session_id = cid.clone();
                    spawn_local(async move {
                        if let Some(storage) = storage::get_storage() {
                            if let Err(e) = storage.end_session(&session_id).await {
                                log::error!("Failed to end session {}: {}", session_id, e);
                            }
                        }
                    });
                }
            }
        });
    }

    // VAD의 speech_end 이벤트 콜백 — 녹음 세그먼트를 잘라 Whisper로 전송
    //
    // Leptos의 `Callback`은 `Send + Sync` 경계를 요구하지만, 우리가 캡처하는
    // `RecorderSlot`은 `Rc<RefCell<_>>`와 web_sys 타입을 담고 있어 `!Send`이다.
    // WASM 단일 스레드 환경이므로 `SendWrapper`로 감싸 경계만 통과시킨다.
    let on_speech_end = {
        let wrapped_slot = SendWrapper::new(recorder_slot.clone());
        Callback::new(move |_| {
            // 콜백은 반응형 컨텍스트 밖이므로 untracked 읽기
            let cid_opt = case_id.get_untracked();
            let slot = wrapped_slot.clone().take();
            let set_transcripts = set_transcripts.clone();
            let set_sentiments = set_sentiments.clone();
            let set_last_analysis = set_last_analysis.clone();
            let set_last_reasoning = set_last_reasoning.clone();

            spawn_local(async move {
                match flush_recorder_and_transcribe(&slot).await {
                    Ok(Some(text)) => {
                        log::info!("🎤 Whisper: {}", text);
                        let ts = js_sys::Date::now();
                        set_transcripts.update(|entries| {
                            entries.push(TranscriptEntry {
                                timestamp: ts,
                                text: text.clone(),
                                speaker: "client".to_string(),
                            });
                        });

                        if let Some(cid) = cid_opt {
                            // 저장소에 저장 + 분석 + 추론
                            let session_id = cid.clone();
                            let text_clone = text.clone();
                            spawn_local(async move {
                                if let Some(storage) = storage::get_storage() {
                                    if let Err(e) = storage.append_transcript(&session_id, &text_clone, "client").await {
                                        log::error!("Failed to save transcript: {}", e);
                                    }
                                }
                            });

                            let text_for_analysis = text.clone();
                            let session_for_analysis = cid.clone();
                            spawn_local(async move {
                                if let Ok(analysis) = analyze_text(&text_for_analysis).await {
                                    set_last_analysis.set(Some(analysis.summary.clone()));
                                    set_sentiments.update(|s| {
                                        s.push(SentimentData {
                                            timestamp: js_sys::Date::now(),
                                            sentiment: analysis.sentiment.clone(),
                                            confidence: analysis.confidence,
                                        });
                                    });
                                    if let Some(storage) = storage::get_storage() {
                                        if let Err(e) = storage
                                            .save_sentiment_analysis(
                                                &session_for_analysis,
                                                &analysis.sentiment,
                                                analysis.confidence,
                                                &text_for_analysis,
                                            )
                                            .await
                                        {
                                            log::error!("Failed to save sentiment: {}", e);
                                        }
                                    }
                                }
                            });

                            let text_for_reasoning = text.clone();
                            let session_for_reasoning = cid.clone();
                            spawn_local(async move {
                                if let Ok(reasoning) = reason_text(&text_for_reasoning).await {
                                    set_last_reasoning.set(Some(reasoning.clone()));
                                    if let Some(storage) = storage::get_storage() {
                                        let reasoning_json =
                                            serde_json::to_string(&reasoning).unwrap_or_default();
                                        if let Err(e) = storage
                                            .save_reasoning_result(
                                                &session_for_reasoning,
                                                &reasoning_json,
                                                &text_for_reasoning,
                                            )
                                            .await
                                        {
                                            log::error!("Failed to save reasoning: {}", e);
                                        }
                                    }
                                }
                            });
                        }
                    }
                    Ok(None) => {
                        log::debug!("Speech end fired but no audio accumulated yet");
                    }
                    Err(e) => {
                        log::error!("STT pipeline error: {}", e);
                    }
                }
            });
        })
    };
    
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
                                    let speaker_color = get_speaker_color(&entry.speaker);
                                    let speaker_name = format_speaker_name(&entry.speaker);
                                    let is_analyst = entry.speaker == "analyst" || entry.speaker == "counselor";
                                    
                                    view! {
                                        <div class=if is_analyst { "flex justify-end" } else { "flex justify-start" }>
                                            <div class="flex items-start gap-1.5 max-w-[90%]">
                                                // 화자 아이콘 (왼쪽)
                                                <div 
                                                    class="w-5 h-5 rounded-full flex items-center justify-center flex-shrink-0 mt-0.5"
                                                    style:background-color=format!("rgb({})", speaker_color)
                                                    title=speaker_name.clone()
                                                >
                                                    <span class="text-white text-[10px] font-bold">
                                                        {speaker_name.chars().next().unwrap_or('?')}
                                                    </span>
                                                </div>
                                                // 메시지 버블
                                                <div class="px-2.5 py-1.5 text-white text-xs rounded border border-grok-border"
                                                    style:background-color=format!("rgba({}, 0.15)", speaker_color)
                                                    style:border-color=format!("rgba({}, 0.3)", speaker_color)
                                                >
                                                    <p class="leading-relaxed">{entry.text.clone()}</p>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        }.into_any()
                    }
                }}
            </div>
            
            // 실시간 입력 표시 + 음성 시각화
            <Show when=move || is_active.get()>
                <div class="mt-3 pt-3 border-t border-grok-border space-y-3">
                    // 🎵 음성 스펙트럼 시각화 + VAD (speech_end → Whisper 호출)
                    <AudioVisualizer
                        is_active=is_active
                        transcript=interim_text
                        on_speech_end=on_speech_end.clone()
                        width=320
                        height=100
                    />
                    
                    // 🔍 마지막 분석 결과
                    <Show when=move || last_analysis.get().is_some()>
                        <div class="bg-grok-gray/50 rounded px-3 py-2 text-sm border border-grok-border">
                            <span class="text-grok-muted text-xs font-mono mr-2">"감정:"</span>
                            <span class="text-white">{move || last_analysis.get().unwrap_or_default()}</span>
                        </div>
                    </Show>
                    
                    // 🧠 Neurosymbolic 추론 결과
                    <Show when=move || last_reasoning.get().is_some()>
                        <div class="bg-gradient-to-r from-purple-900/30 to-blue-900/30 rounded px-3 py-2 text-sm border border-purple-500/30 space-y-1">
                            <div class="flex items-center gap-2">
                                <span class="text-purple-400 text-xs font-mono">"🧠 추론"</span>
                                {move || {
                                    let r = last_reasoning.get();
                                    if let Some(reasoning) = r {
                                        if reasoning.critical {
                                            view! { <span class="text-xs bg-red-500/30 text-red-300 px-1.5 rounded">"⚠️ CRITICAL"</span> }.into_any()
                                        } else if reasoning.priority == "high" {
                                            view! { <span class="text-xs bg-yellow-500/30 text-yellow-300 px-1.5 rounded">"🔶 HIGH"</span> }.into_any()
                                        } else {
                                            view! { <span class="text-xs bg-green-500/30 text-green-300 px-1.5 rounded">"✅ NORMAL"</span> }.into_any()
                                        }
                                    } else {
                                        view! { <span></span> }.into_any()
                                    }
                                }}
                            </div>
                            // 엔티티
                            {move || {
                                let r = last_reasoning.get();
                                if let Some(reasoning) = r {
                                    view! {
                                        <div class="flex flex-wrap gap-1">
                                            {reasoning.entities.iter().map(|e| {
                                                let type_color = match e.entity_type.as_str() {
                                                    t if t.contains("Problem") => "bg-red-500/20 text-red-300",
                                                    t if t.contains("Cause") => "bg-orange-500/20 text-orange-300",
                                                    t if t.contains("Solution") => "bg-green-500/20 text-green-300",
                                                    t if t.contains("Impact") => "bg-yellow-500/20 text-yellow-300",
                                                    _ => "bg-gray-500/20 text-gray-300",
                                                };
                                                view! {
                                                    <span class=format!("text-xs px-1.5 py-0.5 rounded {}", type_color)>
                                                        {e.text.clone()}
                                                    </span>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }
                            }}
                            // 해결책
                            {move || {
                                let r = last_reasoning.get();
                                if let Some(reasoning) = r {
                                    if !reasoning.solutions.is_empty() {
                                        view! {
                                            <div class="text-xs text-green-400">
                                                {"💡 "}
                                                {reasoning.solutions.iter().map(|s| {
                                                    let status = if s.blocked { "🚫" } else if s.permanent { "⭐" } else { "✓" };
                                                    format!("{} {}", s.name, status)
                                                }).collect::<Vec<_>>().join(", ")}
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! { <div></div> }.into_any()
                                    }
                                } else {
                                    view! { <div></div> }.into_any()
                                }
                            }}
                        </div>
                    </Show>
                    
                    <div class="flex items-center gap-2 text-grok-muted">
                        <div class="flex gap-0.5">
                            <div class="w-1 h-1 bg-grok-blue rounded-full animate-bounce" style="animation-delay: 0ms" />
                            <div class="w-1 h-1 bg-grok-blue rounded-full animate-bounce" style="animation-delay: 100ms" />
                            <div class="w-1 h-1 bg-grok-blue rounded-full animate-bounce" style="animation-delay: 200ms" />
                        </div>
                        <span class="text-xs font-mono">"listening"</span>
                        
                        // 🔘 수동 분석 버튼 (항상 표시)
                        <button
                            class=move || {
                                let has_text = !interim_text.get().is_empty();
                                if has_text {
                                    "ml-auto px-3 py-1 bg-grok-blue hover:bg-blue-600 text-white text-xs rounded transition-colors cursor-pointer"
                                } else {
                                    "ml-auto px-3 py-1 bg-gray-600 text-gray-400 text-xs rounded cursor-not-allowed"
                                }
                            }
                            disabled=move || interim_text.get().is_empty()
                            on:click=move |_| {
                                let text = interim_text.get();
                                if !text.is_empty() {
                                    // 최종 결과로 처리
                                    let text_clone = text.clone();
                                    set_transcripts.update(|entries| {
                                        entries.push(TranscriptEntry {
                                            timestamp: js_sys::Date::now(),
                                            text: text_clone,
                                            speaker: "client".to_string(),
                                        });
                                    });
                                    set_interim_text.set(String::new());
                                    
                                    // 📝 저장소에 대화 저장
                                    if let Some(session_id) = case_id.get() {
                                        let session_id_clone = session_id.clone();
                                        let text_for_storage = text.clone();
                                        spawn_local(async move {
                                            if let Some(storage) = storage::get_storage() {
                                                if let Err(e) = storage.append_transcript(&session_id_clone, &text_for_storage, "client").await {
                                                    log::error!("Failed to save transcript: {}", e);
                                                }
                                            }
                                        });
                                        
                                        // 분석 + 추론 실행
                                        let text_for_analysis = text.clone();
                                        let text_for_reasoning = text.clone();
                                        let session_id_for_analysis = session_id.clone();
                                        let session_id_for_reasoning = session_id.clone();
                                        
                                        spawn_local(async move {
                                            if let Ok(analysis) = analyze_text(&text_for_analysis).await {
                                                set_last_analysis.set(Some(analysis.summary.clone()));
                                                set_sentiments.update(|s| {
                                                    s.push(SentimentData {
                                                        timestamp: js_sys::Date::now(),
                                                        sentiment: analysis.sentiment.clone(),
                                                        confidence: analysis.confidence,
                                                    });
                                                });
                                                
                                                // 📊 감정 분석 결과 저장
                                                if let Some(storage) = storage::get_storage() {
                                                    if let Err(e) = storage.save_sentiment_analysis(
                                                        &session_id_for_analysis, 
                                                        &analysis.sentiment, 
                                                        analysis.confidence, 
                                                        &text_for_analysis
                                                    ).await {
                                                        log::error!("Failed to save sentiment: {}", e);
                                                    }
                                                }
                                            }
                                        });
                                        
                                        let set_reasoning = set_last_reasoning.clone();
                                        spawn_local(async move {
                                            if let Ok(reasoning) = reason_text(&text_for_reasoning).await {
                                                set_reasoning.set(Some(reasoning.clone()));
                                                
                                                // 🧠 추론 결과 저장
                                                if let Some(storage) = storage::get_storage() {
                                                    let reasoning_json = serde_json::to_string(&reasoning).unwrap_or_default();
                                                    if let Err(e) = storage.save_reasoning_result(
                                                        &session_id_for_reasoning, 
                                                        &reasoning_json, 
                                                        &text_for_reasoning
                                                    ).await {
                                                        log::error!("Failed to save reasoning: {}", e);
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                        >
                            "🔍 분석하기"
                        </button>
                    </div>
                </div>
            </Show>
        </div>
    }
}

// ───────────────────────── 녹음 파이프라인 ─────────────────────────

/// 마이크 열고 MediaRecorder 시작. 이후 VAD의 speech_end 이벤트가 올 때까지 계속 녹음.
async fn start_recorder(slot: &RecorderSlot) -> Result<(), String> {
    let window = web_sys::window().ok_or("no window")?;
    let navigator = window.navigator();
    let media_devices = navigator
        .media_devices()
        .map_err(|e| format!("media_devices: {:?}", e))?;

    // 노이즈/에코 제거 ON — 한국어 STT 품질에 직결됨
    let constraints = web_sys::MediaStreamConstraints::new();
    let audio_cfg = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&audio_cfg, &"noiseSuppression".into(), &JsValue::TRUE);
    let _ = js_sys::Reflect::set(&audio_cfg, &"echoCancellation".into(), &JsValue::TRUE);
    let _ = js_sys::Reflect::set(&audio_cfg, &"autoGainControl".into(), &JsValue::TRUE);
    constraints.set_audio(&audio_cfg.into());
    constraints.set_video(&JsValue::FALSE);

    let stream_promise = media_devices
        .get_user_media_with_constraints(&constraints)
        .map_err(|e| format!("getUserMedia call: {:?}", e))?;
    let stream: MediaStream = JsFuture::from(stream_promise)
        .await
        .map_err(|e| format!("getUserMedia await: {:?}", e))?
        .dyn_into()
        .map_err(|_| "MediaStream cast failed")?;

    let recorder = build_recorder(&stream, slot)?;
    recorder.start().map_err(|e| format!("recorder.start: {:?}", e))?;

    *slot.stream.borrow_mut() = Some(stream);
    *slot.recorder.borrow_mut() = Some(recorder);
    log::info!("MediaRecorder started (mime={})", RECORDER_MIME);
    Ok(())
}

/// MediaRecorder 생성 — ondataavailable 콜백을 slot.chunks에 연결.
fn build_recorder(stream: &MediaStream, slot: &RecorderSlot) -> Result<MediaRecorder, String> {
    // WebM/Opus 지원 확인 후 시도. 미지원 환경(Safari 일부)은 기본 MIME으로 폴백.
    let use_webm = MediaRecorder::is_type_supported(RECORDER_MIME);
    let recorder = if use_webm {
        let opts = MediaRecorderOptions::new();
        opts.set_mime_type(RECORDER_MIME);
        MediaRecorder::new_with_media_stream_and_media_recorder_options(stream, &opts)
            .map_err(|e| format!("MediaRecorder::new (webm): {:?}", e))?
    } else {
        MediaRecorder::new_with_media_stream(stream)
            .map_err(|e| format!("MediaRecorder::new (default): {:?}", e))?
    };

    let chunks = slot.chunks.clone();
    let on_data = Closure::wrap(Box::new(move |event: BlobEvent| {
        if let Some(blob) = event.data() {
            if blob.size() > 0.0 {
                chunks.borrow_mut().push(blob);
            }
        }
    }) as Box<dyn FnMut(BlobEvent)>);
    recorder.set_ondataavailable(Some(on_data.as_ref().unchecked_ref()));
    on_data.forget();

    Ok(recorder)
}

/// 세션 종료 시 MediaRecorder와 MediaStream을 깨끗이 해제.
fn stop_recorder(slot: &RecorderSlot) {
    if let Some(recorder) = slot.recorder.borrow_mut().take() {
        if recorder.state() == web_sys::RecordingState::Recording {
            let _ = recorder.stop();
        }
    }
    if let Some(stream) = slot.stream.borrow_mut().take() {
        let tracks = stream.get_tracks();
        for i in 0..tracks.length() {
            if let Ok(track) = tracks.get(i).dyn_into::<web_sys::MediaStreamTrack>() {
                track.stop();
            }
        }
    }
    slot.chunks.borrow_mut().clear();
}

/// VAD speech_end 시 호출 — 현재까지 녹음된 Blob을 모아 Whisper로 보내고,
/// 다음 발화를 위해 새 MediaRecorder를 즉시 재시작한다.
async fn flush_recorder_and_transcribe(slot: &RecorderSlot) -> Result<Option<String>, String> {
    // 1. 현재 녹음기 중지 → ondataavailable이 마지막 청크를 push할 때까지 onstop 대기
    let old_recorder = slot.recorder.borrow_mut().take();
    if let Some(recorder) = old_recorder {
        if recorder.state() == web_sys::RecordingState::Recording {
            wait_for_recorder_stop(&recorder).await?;
        }
    }

    // 2. 누적된 chunk를 하나의 Blob으로 결합
    let chunks = std::mem::take(&mut *slot.chunks.borrow_mut());
    if chunks.is_empty() {
        // 다음 발화를 위해 재시작만 하고 반환
        restart_recorder(slot)?;
        return Ok(None);
    }

    let blob_parts = js_sys::Array::new();
    for chunk in &chunks {
        blob_parts.push(chunk);
    }
    let combined = Blob::new_with_blob_sequence_and_options(
        &blob_parts,
        web_sys::BlobPropertyBag::new().type_(RECORDER_MIME),
    )
    .map_err(|e| format!("Blob combine: {:?}", e))?;

    // 3. 다음 발화를 위해 녹음 재시작 (네트워크 지연과 병렬)
    restart_recorder(slot)?;

    // 4. Whisper로 업로드
    let text = upload_to_whisper(combined).await?;
    if text.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

/// MediaRecorder.stop()을 호출하고 onstop 이벤트를 대기.
async fn wait_for_recorder_stop(recorder: &MediaRecorder) -> Result<(), String> {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let once = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::NULL);
        });
        recorder.set_onstop(Some(once.unchecked_ref()));
    });
    recorder.stop().map_err(|e| format!("recorder.stop: {:?}", e))?;
    JsFuture::from(promise)
        .await
        .map_err(|e| format!("onstop await: {:?}", e))?;
    Ok(())
}

/// 동일 MediaStream으로 새 MediaRecorder를 만들어 연속 녹음.
fn restart_recorder(slot: &RecorderSlot) -> Result<(), String> {
    let stream_opt = slot.stream.borrow().clone();
    let stream = stream_opt.ok_or("MediaStream missing, cannot restart recorder")?;
    let recorder = build_recorder(&stream, slot)?;
    recorder.start().map_err(|e| format!("recorder.start: {:?}", e))?;
    *slot.recorder.borrow_mut() = Some(recorder);
    Ok(())
}

/// Blob을 multipart/form-data로 `/api/stt`에 업로드하고 인식 결과를 받음.
async fn upload_to_whisper(audio: Blob) -> Result<String, String> {
    let form = FormData::new().map_err(|e| format!("FormData: {:?}", e))?;
    // 파일명의 확장자로 서버가 포맷을 파악 (webm)
    form.append_with_blob_and_filename("audio", &audio, "speech.webm")
        .map_err(|e| format!("FormData append: {:?}", e))?;

    let window = web_sys::window().ok_or("no window")?;
    let req_init = web_sys::RequestInit::new();
    req_init.set_method("POST");
    req_init.set_body(&form);

    let request = web_sys::Request::new_with_str_and_init(STT_ENDPOINT, &req_init)
        .map_err(|e| format!("Request::new: {:?}", e))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("fetch: {:?}", e))?;
    let resp: web_sys::Response = resp_value.dyn_into().map_err(|_| "Response cast")?;
    if !resp.ok() {
        return Err(format!("STT HTTP {}", resp.status()));
    }

    let json_promise = resp.json().map_err(|e| format!("resp.json(): {:?}", e))?;
    let json_value = JsFuture::from(json_promise)
        .await
        .map_err(|e| format!("json await: {:?}", e))?;

    let parsed: SttResponse = serde_wasm_bindgen::from_value(json_value)
        .map_err(|e| format!("STT response parse: {:?}", e))?;
    Ok(parsed.text)
}


/// 분석 결과 구조체
#[derive(Clone, Debug, serde::Deserialize)]
struct AnalysisResult {
    pub sentiment: String,
    pub confidence: f64,
    pub intent: String,
    pub summary: String,
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// 문장 분석 API 호출
async fn analyze_text(text: &str) -> Result<AnalysisResult, String> {
    use gloo_net::http::Request;
    
    let body = serde_json::json!({
        "text": text,
        "timestamp": js_sys::Date::now()
    });
    
    // 직접 API 서버 호출 (프록시 우회)
    let response = Request::post("http://localhost:8080/api/analyze")
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    if response.ok() {
        response.json::<AnalysisResult>().await.map_err(|e| e.to_string())
    } else {
        Err(format!("API error: {}", response.status()))
    }
}

/// Neurosymbolic 추론 결과
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ReasoningResult {
    pub input: String,
    pub entities: Vec<EntityInfo>,
    pub relations: Vec<RelationInfo>,
    pub solutions: Vec<SolutionInfo>,
    pub critical: bool,
    pub priority: String,
    pub summary: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EntityInfo {
    pub text: String,
    #[serde(rename = "type")]
    pub entity_type: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct RelationInfo {
    pub name: String,
    pub subject: String,
    pub object: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SolutionInfo {
    pub name: String,
    pub problem: String,
    pub blocked: bool,
    #[serde(rename = "highPriority")]
    pub high_priority: bool,
    pub permanent: bool,
}

/// Neurosymbolic 추론 API 호출
pub async fn reason_text(text: &str) -> Result<ReasoningResult, String> {
    use gloo_net::http::Request;
    
    let body = serde_json::json!({
        "text": text
    });
    
    // 직접 API 서버 호출 (프록시 우회)
    let response = Request::post("http://localhost:8080/api/reason")
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    if response.ok() {
        response.json::<ReasoningResult>().await.map_err(|e| e.to_string())
    } else {
        Err(format!("API error: {}", response.status()))
    }
}

/// 화자 ID에 따른 RGB 색상 반환
fn get_speaker_color(speaker_id: &str) -> String {
    match speaker_id {
        "speaker_low" | "Speaker A" => "59, 130, 246".to_string(), // blue
        "speaker_mid" | "Speaker B" => "239, 68, 68".to_string(),  // red
        "speaker_high" | "Speaker C" => "16, 185, 129".to_string(), // green
        "analyst" | "counselor" => "139, 92, 246".to_string(),       // purple
        "client" => "245, 158, 11".to_string(),                      // yellow
        "speaker_unknown" => "156, 163, 175".to_string(),            // gray
        _ => {
            // 기타 화자는 해시 기반 색상
            let hash = speaker_id.chars()
                .map(|c| c as usize)
                .sum::<usize>();
            
            let colors = [
                "59, 130, 246",   // blue
                "239, 68, 68",    // red
                "16, 185, 129",   // green
                "245, 158, 11",   // yellow
                "139, 92, 246",   // purple
                "249, 115, 22",   // orange
                "6, 182, 212",    // cyan
                "132, 204, 22",   // lime
            ];
            
            colors[hash % colors.len()].to_string()
        }
    }
}

/// 화자 ID를 사용자 친화적 이름으로 변환
fn format_speaker_name(speaker_id: &str) -> String {
    match speaker_id {
        "speaker_low" => "Speaker A".to_string(),
        "speaker_mid" => "Speaker B".to_string(),
        "speaker_high" => "Speaker C".to_string(),
        "speaker_unknown" => "Unknown".to_string(),
        "analyst" => "Analyst".to_string(),
        "counselor" => "Counselor".to_string(),
        "client" => "Client".to_string(),
        _ => speaker_id.replace("speaker_", "Speaker ").to_uppercase(),
    }
}
