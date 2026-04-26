use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{AudioBuffer, AudioContext, Blob, BlobEvent, FormData, MediaRecorder, MediaRecorderOptions, MediaStream};
use send_wrapper::SendWrapper;
use gloo_timers::callback::Interval;
use std::cell::RefCell;
use std::rc::Rc;
use crate::app::{TranscriptEntry, SentimentData};
use crate::components::icons;
use crate::components::audio_visualizer::AudioVisualizer;
use crate::components::resemblyzer::ResemblyzerEngine;
use crate::storage;

/// 백엔드 STT 엔드포인트.
/// Origin-relative — `vla.coreon.build`(production) / `localhost:8081`(Trunk dev) / SSH 터널 모두 동일하게 동작.
/// dev에서는 Trunk.toml의 `[[proxy]] /api/`가 8080으로 포워딩하고,
/// production에서는 cloudflared ingress가 같은 path를 8080으로 라우팅한다.
const STT_ENDPOINT: &str = "/api/stt";

/// MediaRecorder 선호 MIME 타입 (WebM/Opus는 Whisper가 직접 지원).
const RECORDER_MIME: &str = "audio/webm;codecs=opus";

/// Interim flush 주기 — 발화 중에도 이 간격으로 partial 텍스트를 표시.
/// 짧을수록 latency 줄지만 Whisper 호출 빈도와 boundary cut 위험 증가.
const INTERIM_INTERVAL_MS: u32 = 2000;

/// 녹음기 세션 상태 — speech_end 콜백과 MediaRecorder 사이에서 공유.
#[derive(Clone, Default)]
struct RecorderSlot {
    recorder: Rc<RefCell<Option<MediaRecorder>>>,
    stream: Rc<RefCell<Option<MediaStream>>>,
    /// 현재 녹음 세그먼트의 Blob 청크들.
    chunks: Rc<RefCell<Vec<Blob>>>,
    /// 현재 발화에서 interim flush로 누적한 텍스트. speech_end 시 한 번에 transcripts로 확정.
    pending_text: Rc<RefCell<String>>,
    /// 현재 발화에서 interim flush로 누적한 16kHz f32 mono PCM.
    /// speech_end 시 이 전체를 ResemblyzerEngine::identify_speaker에 넘겨 화자 식별.
    pending_pcm: Rc<RefCell<Vec<f32>>>,
    /// 동시 flush 가드 — 진행 중이면 새 interim 호출은 스킵해 recorder 상태 꼬임 방지.
    flushing: Rc<RefCell<bool>>,
    /// 주기적 interim flush 타이머. Drop 시 setInterval 자동 해제 (RAII).
    interim_timer: Rc<RefCell<Option<Interval>>>,
}

/// flush_recorder_and_transcribe 반환 — STT 결과 + 화자 식별용 PCM.
struct FlushOutput {
    text: Option<String>,
    pcm_16k: Option<Vec<f32>>,
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

    // 화자 식별 엔진 — Resemblyzer ONNX 임베딩 + 코사인 유사도 매칭.
    // 컴포넌트 인스턴스 하나당 한 엔진을 유지해 발화 간 화자 레지스트리(speaker_embeddings)를 누적.
    // ONNX 모델 자체는 resemblyzer.rs의 `static MODEL: OnceLock`으로 전역 1회만 로드.
    let resemblyzer = Rc::new(RefCell::new(ResemblyzerEngine::new()));
    
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

                    // Interim flush 타이머 — 2초마다 현재 chunk를 Whisper로 보내고
                    // pending_text에 누적, interim_text 시그널을 갱신해 라이브 표시.
                    let timer_slot = recorder_slot.clone();
                    let interval = Interval::new(INTERIM_INTERVAL_MS, move || {
                        let slot = timer_slot.clone();
                        spawn_local(async move {
                            run_interim_flush(&slot, set_interim_text).await;
                        });
                    });
                    *recorder_slot.interim_timer.borrow_mut() = Some(interval);
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

    // VAD의 speech_end 이벤트 콜백 — 발화의 마지막 chunk를 변환하고
    // interim flush로 누적된 pending_text/pending_pcm과 합쳐 한 번에 transcripts로 확정한다.
    // 누적 PCM으로 ResemblyzerEngine에 화자 식별을 요청, 결과를 transcript 엔트리에 반영.
    //
    // Leptos의 `Callback`은 `Send + Sync` 경계를 요구하지만, 우리가 캡처하는
    // `RecorderSlot`/`Rc<RefCell<ResemblyzerEngine>>`은 `!Send`이다.
    // WASM 단일 스레드 환경이므로 `SendWrapper`로 감싸 경계만 통과시킨다.
    let on_speech_end = {
        let wrapped_slot = SendWrapper::new(recorder_slot.clone());
        let wrapped_resemblyzer = SendWrapper::new(resemblyzer.clone());
        Callback::new(move |_| {
            // 콜백은 반응형 컨텍스트 밖이므로 untracked 읽기
            let cid_opt = case_id.get_untracked();
            let slot = wrapped_slot.clone().take();
            let engine = wrapped_resemblyzer.clone().take();
            let set_transcripts = set_transcripts.clone();
            let set_sentiments = set_sentiments.clone();
            let set_last_analysis = set_last_analysis.clone();
            let set_last_reasoning = set_last_reasoning.clone();

            spawn_local(async move {
                // 1. 마지막 tail audio (직전 interim flush 이후 0~2s) 변환.
                //    Interim과 동시 호출되면 flushing 가드 풀릴 때까지 짧게 yield.
                while *slot.flushing.borrow() {
                    gloo_timers::future::TimeoutFuture::new(20).await;
                }
                *slot.flushing.borrow_mut() = true;
                let tail = flush_recorder_and_transcribe(&slot).await;
                *slot.flushing.borrow_mut() = false;

                match tail {
                    Ok(out) => {
                        if let Some(pcm) = out.pcm_16k {
                            slot.pending_pcm.borrow_mut().extend_from_slice(&pcm);
                        }
                        if let Some(text) = out.text {
                            let trimmed = text.trim();
                            if !trimmed.is_empty() {
                                let mut pending = slot.pending_text.borrow_mut();
                                if !pending.is_empty() {
                                    pending.push(' ');
                                }
                                pending.push_str(trimmed);
                            }
                        }
                    }
                    Err(e) => log::error!("STT pipeline error: {}", e),
                }

                // 2. 누적된 발화 전체를 확정하고 interim 표시 클리어.
                let final_text = std::mem::take(&mut *slot.pending_text.borrow_mut());
                let final_pcm = std::mem::take(&mut *slot.pending_pcm.borrow_mut());
                set_interim_text.set(String::new());
                let final_text = final_text.trim().to_string();
                if final_text.is_empty() {
                    log::debug!("Speech end: no text accumulated");
                    return;
                }

                log::info!("🎤 Whisper (final): {}", final_text);

                // 3. 화자 식별 — 누적 PCM 전체를 ResemblyzerEngine에 넘김.
                //    None 반환(오디오 너무 짧거나 임베딩 실패) 시 current_speaker prop fallback,
                //    그것도 None이면 "client"로 최종 fallback.
                let speaker_from_engine = if final_pcm.len() >= 16000 / 2 {
                    // 최소 0.5초 이상 PCM일 때만 식별 시도 (짧으면 fallback embedding이 None 반환).
                    engine.borrow_mut().identify_speaker(&final_pcm)
                } else {
                    None
                };
                let speaker = speaker_from_engine
                    .or_else(|| current_speaker.and_then(|s| s.get_untracked()))
                    .unwrap_or_else(|| "client".to_string());
                log::info!("👤 Speaker: {} (PCM {} samples)", speaker, final_pcm.len());
                let ts = js_sys::Date::now();
                set_transcripts.update(|entries| {
                    entries.push(TranscriptEntry {
                        timestamp: ts,
                        text: final_text.clone(),
                        speaker: speaker.clone(),
                    });
                });

                // 3. 저장 + 분석 + 추론 — 누적된 전체 발화를 한 번에 처리.
                if let Some(cid) = cid_opt {
                    let session_id = cid.clone();
                    let text_clone = final_text.clone();
                    let speaker_for_storage = speaker.clone();
                    spawn_local(async move {
                        if let Some(storage) = storage::get_storage() {
                            if let Err(e) = storage.append_transcript(&session_id, &text_clone, &speaker_for_storage).await {
                                log::error!("Failed to save transcript: {}", e);
                            }
                        }
                    });

                    let text_for_analysis = final_text.clone();
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

                    let text_for_reasoning = final_text.clone();
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
            });
        })
    };

    // 수동 분석 버튼이 interim_text를 확정할 때 pending_text도 같이 비워야
    // 다음 speech_end가 같은 텍스트를 한 번 더 push하지 않는다.
    // Leptos view 클로저는 Send/Sync를 요구하므로 !Send인 Rc는 SendWrapper로 통과.
    let pending_text_for_button = SendWrapper::new(recorder_slot.pending_text.clone());

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
                            on:click={
                                // 부모 children-closure가 Fn이어야 하므로
                                // 클릭 클로저 진입 직전에 SendWrapper를 한 번 clone해 그 사본만 move.
                                let pending = pending_text_for_button.clone();
                                move |_| {
                                let text = interim_text.get();
                                if !text.is_empty() {
                                    // SpeakerDiarization 결과 반영 — speech_end 경로와 동일한 fallback 정책.
                                    let speaker = current_speaker
                                        .and_then(|s| s.get_untracked())
                                        .unwrap_or_else(|| "client".to_string());
                                    // 최종 결과로 처리
                                    let text_clone = text.clone();
                                    let speaker_clone = speaker.clone();
                                    set_transcripts.update(|entries| {
                                        entries.push(TranscriptEntry {
                                            timestamp: js_sys::Date::now(),
                                            text: text_clone,
                                            speaker: speaker_clone,
                                        });
                                    });
                                    set_interim_text.set(String::new());
                                    // pending_text도 비워야 다음 speech_end가 중복 push하지 않음
                                    pending.clone().take().borrow_mut().clear();

                                    // 📝 저장소에 대화 저장
                                    if let Some(session_id) = case_id.get() {
                                        let session_id_clone = session_id.clone();
                                        let text_for_storage = text.clone();
                                        let speaker_for_storage = speaker.clone();
                                        spawn_local(async move {
                                            if let Some(storage) = storage::get_storage() {
                                                if let Err(e) = storage.append_transcript(&session_id_clone, &text_for_storage, &speaker_for_storage).await {
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
    // Interim 타이머 먼저 해제 — drop 시 setInterval 자동 cancel.
    slot.interim_timer.borrow_mut().take();

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
    slot.pending_text.borrow_mut().clear();
    slot.pending_pcm.borrow_mut().clear();
    *slot.flushing.borrow_mut() = false;
}

/// 주기적 interim flush — 2초마다 현재 chunk를 Whisper STT + PCM 디코드 후
/// pending_text/pending_pcm에 누적. interim_text 시그널을 라이브로 갱신.
/// 화자 식별은 여기서 안 함 (interim은 짧고 발화 단위로 묶어 speech_end에서 한 번에 처리).
/// 동시 flush(speech_end와 충돌 등)는 `flushing` 가드로 스킵.
async fn run_interim_flush(slot: &RecorderSlot, set_interim_text: WriteSignal<String>) {
    if *slot.flushing.borrow() {
        return;
    }
    // chunk가 아직 없으면(녹음 막 시작) 굳이 stop/restart 비용을 치를 필요 없음.
    if slot.recorder.borrow().is_none() {
        return;
    }
    *slot.flushing.borrow_mut() = true;
    let result = flush_recorder_and_transcribe(slot).await;
    *slot.flushing.borrow_mut() = false;

    let Ok(out) = result else { return };

    // PCM은 텍스트 유무와 무관하게 누적 (잠깐의 무음/잡음에서도 화자 임베딩에 기여).
    if let Some(pcm) = out.pcm_16k {
        slot.pending_pcm.borrow_mut().extend_from_slice(&pcm);
    }

    if let Some(text) = out.text {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let snapshot = {
            let mut pending = slot.pending_text.borrow_mut();
            if !pending.is_empty() {
                pending.push(' ');
            }
            pending.push_str(trimmed);
            pending.clone()
        };
        log::debug!("Interim flush: {}", snapshot);
        set_interim_text.set(snapshot);
    }
}

/// VAD speech_end / 주기적 interim flush 시 호출 — 현재까지 녹음된 Blob을 모아
/// (a) Whisper STT 변환 + (b) 화자 식별용 16kHz PCM 디코드 둘 다 수행하고,
/// 다음 발화를 위해 새 MediaRecorder를 즉시 재시작한다.
async fn flush_recorder_and_transcribe(slot: &RecorderSlot) -> Result<FlushOutput, String> {
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
        return Ok(FlushOutput { text: None, pcm_16k: None });
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

    // 4. PCM 디코드 (화자 식별용) — 실패해도 STT는 계속 시도.
    //    Blob.array_buffer는 데이터를 소비하지 않으므로 같은 Blob을 STT 업로드에도 재사용 가능.
    let pcm_16k = match decode_blob_to_pcm_16k(&combined).await {
        Ok(pcm) => Some(pcm),
        Err(e) => {
            log::warn!("PCM decode failed (화자 식별 스킵): {}", e);
            None
        }
    };

    // 5. Whisper로 업로드
    let text_raw = upload_to_whisper(combined).await?;
    let text = if text_raw.trim().is_empty() { None } else { Some(text_raw) };

    Ok(FlushOutput { text, pcm_16k })
}

/// WebM/Opus Blob을 디코드하고 16kHz f32 mono PCM으로 변환.
/// AudioContext.decodeAudioData가 컨테이너 포맷을 자동 인식 (Opus/WebM/MP3 등).
/// AudioBuffer의 sample_rate는 source rate (보통 MediaRecorder는 48000Hz) → 16kHz로 다운샘플.
async fn decode_blob_to_pcm_16k(blob: &Blob) -> Result<Vec<f32>, String> {
    // Blob → ArrayBuffer
    let array_buffer_promise = blob.array_buffer();
    let array_buffer: js_sys::ArrayBuffer = JsFuture::from(array_buffer_promise)
        .await
        .map_err(|e| format!("blob.arrayBuffer: {:?}", e))?
        .dyn_into()
        .map_err(|_| "ArrayBuffer cast failed".to_string())?;

    // AudioContext 생성 후 decodeAudioData. 디코드 후 close.
    let ctx = AudioContext::new().map_err(|e| format!("AudioContext::new: {:?}", e))?;
    let decode_promise = ctx
        .decode_audio_data(&array_buffer)
        .map_err(|e| format!("decode_audio_data start: {:?}", e))?;
    let buffer: AudioBuffer = JsFuture::from(decode_promise)
        .await
        .map_err(|e| format!("decode_audio_data await: {:?}", e))?
        .dyn_into()
        .map_err(|_| "AudioBuffer cast failed".to_string())?;

    let source_rate = buffer.sample_rate();
    let pcm = buffer
        .get_channel_data(0)
        .map_err(|e| format!("getChannelData: {:?}", e))?;
    let _ = ctx.close();

    if (source_rate - 16000.0).abs() < 1.0 {
        Ok(pcm)
    } else {
        Ok(downsample_to_16k(&pcm, source_rate))
    }
}

/// 단순 nearest-neighbor 다운샘플 (resemblyzer.rs/speaker_diarization.rs와 동일 알고리즘).
/// 화자 임베딩에는 nearest-neighbor로 충분 (anti-aliasing은 정확도에 거의 영향 없음).
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
    
    // Origin-relative — Trunk proxy / cloudflared ingress가 8080으로 포워딩.
    let response = Request::post("/api/analyze")
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
    
    // Origin-relative — Trunk proxy / cloudflared ingress가 8080으로 포워딩.
    let response = Request::post("/api/reason")
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
