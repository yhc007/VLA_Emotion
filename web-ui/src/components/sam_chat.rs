use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{SpeechRecognition, SpeechRecognitionEvent, SpeechSynthesisUtterance};
use std::sync::atomic::{AtomicBool, Ordering};
use super::audio_visualizer::AudioVisualizer;

// 전역 TTS 상태 (에코 방지)
static IS_SPEAKING: AtomicBool = AtomicBool::new(false);

/// 샘(Sam) 채팅 컴포넌트 - 음성으로 대화하는 🦊 버튼
#[component]
pub fn SamChat() -> impl IntoView {
    let (is_open, set_is_open) = signal(false);
    let (is_listening, set_is_listening) = signal(false);
    let (is_tts_playing, set_is_tts_playing) = signal(false);
    let (messages, set_messages) = signal(Vec::<ChatMessage>::new());
    let (input_text, set_input_text) = signal(String::new());
    let (is_loading, set_is_loading) = signal(false);
    let (interim_transcript, set_interim_transcript) = signal(String::new());

    // 음성 인식 시작 (TTS 재생 중이면 무시)
    let start_listening = move |_| {
        // TTS 재생 중이면 음성 인식 시작하지 않음 (에코 방지)
        if is_tts_playing.get() || IS_SPEAKING.load(Ordering::SeqCst) {
            return;
        }
        
        #[cfg(target_arch = "wasm32")]
        {
            // 브라우저 TTS 상태도 한번 더 체크
            if let Some(window) = web_sys::window() {
                if let Ok(synthesis) = window.speech_synthesis() {
                    if synthesis.speaking() || synthesis.pending() {
                        return;
                    }
                }
            }
        }
        
        set_is_listening.set(true);
        
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(recognition) = SpeechRecognition::new() {
                recognition.set_lang("ko-KR");
                let _ = recognition.set_continuous(false);
                let _ = recognition.set_interim_results(true);  // interim 활성화

                let set_input = set_input_text.clone();
                let set_listening = set_is_listening.clone();
                let set_interim = set_interim_transcript.clone();
                
                let onresult = Closure::wrap(Box::new(move |event: SpeechRecognitionEvent| {
                    if let Some(results) = event.results() {
                        if let Some(result) = results.get(0) {
                            if let Some(alt) = result.get(0) {
                                let transcript = alt.transcript();
                                
                                if result.is_final() {
                                    // 최종 결과
                                    set_input.set(transcript);
                                    set_interim.set(String::new());
                                    set_listening.set(false);
                                } else {
                                    // interim 결과 (실시간)
                                    set_interim.set(transcript);
                                }
                            }
                        }
                    }
                }) as Box<dyn FnMut(SpeechRecognitionEvent)>);

                let set_listening2 = set_is_listening.clone();
                let onerror = Closure::wrap(Box::new(move |_: web_sys::Event| {
                    set_listening2.set(false);
                }) as Box<dyn FnMut(web_sys::Event)>);

                let set_listening3 = set_is_listening.clone();
                let onend = Closure::wrap(Box::new(move |_: web_sys::Event| {
                    set_listening3.set(false);
                }) as Box<dyn FnMut(web_sys::Event)>);

                recognition.set_onresult(Some(onresult.as_ref().unchecked_ref()));
                recognition.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                recognition.set_onend(Some(onend.as_ref().unchecked_ref()));
                
                let _ = recognition.start();
                
                onresult.forget();
                onerror.forget();
                onend.forget();
            }
        }
    };

    // 메시지 전송
    let send_message = {
        let set_is_tts_playing = set_is_tts_playing.clone();
        move |_| {
            let text = input_text.get();
            if text.is_empty() { return; }
            
            // 사용자 메시지 추가
            set_messages.update(|msgs| {
                msgs.push(ChatMessage {
                    role: "user".to_string(),
                    content: text.clone(),
                });
            });
            set_input_text.set(String::new());
            set_is_loading.set(true);

            let set_tts = set_is_tts_playing.clone();
            
            // API 호출
            spawn_local(async move {
                match send_to_sam(&text).await {
                    Ok(response) => {
                        set_messages.update(|msgs| {
                            msgs.push(ChatMessage {
                                role: "assistant".to_string(),
                                content: response.clone(),
                            });
                        });
                        // TTS로 응답 읽기
                        speak_with_callback(&response, set_tts);
                    }
                    Err(e) => {
                        set_messages.update(|msgs| {
                            msgs.push(ChatMessage {
                                role: "assistant".to_string(),
                                content: format!("오류: {}", e),
                            });
                        });
                    }
                }
                set_is_loading.set(false);
            });
        }
    };

    // 마이크 버튼 비활성화 여부
    let mic_disabled = move || is_tts_playing.get() || is_loading.get();

    view! {
        // 🦊 플로팅 버튼
        <button
            class="fixed bottom-6 right-6 w-14 h-14 rounded-full bg-gradient-to-br from-orange-400 to-orange-600 shadow-lg hover:shadow-xl transition-all duration-200 flex items-center justify-center text-2xl z-50 hover:scale-110"
            class:ring-4=is_open
            class:ring-orange-300=is_open
            on:click=move |_| set_is_open.update(|v| *v = !*v)
            title="샘한테 말하기"
        >
            "🦊"
        </button>

        // 채팅 패널
        <Show when=move || is_open.get()>
            <div class="fixed bottom-24 right-6 w-80 h-96 bg-gray-900 rounded-2xl shadow-2xl border border-gray-700 flex flex-col z-50 overflow-hidden">
                // 헤더
                <div class="px-4 py-3 bg-gradient-to-r from-orange-500 to-orange-600 flex items-center gap-2">
                    <span class="text-xl">"🦊"</span>
                    <span class="text-white font-semibold">"샘 (Sam)"</span>
                    <Show when=move || is_tts_playing.get()>
                        <span class="ml-2 text-xs bg-white/20 px-2 py-0.5 rounded-full animate-pulse">"🔊 말하는 중"</span>
                    </Show>
                    <span class="ml-auto text-xs text-orange-200">"AI Assistant"</span>
                </div>

                // 메시지 영역
                <div class="flex-1 overflow-y-auto p-3 space-y-2">
                    <For
                        each=move || messages.get()
                        key=|msg| format!("{}-{}", msg.role, msg.content.len())
                        children=move |msg| {
                            let is_user = msg.role == "user";
                            view! {
                                <div class=move || if is_user { "flex justify-end" } else { "flex justify-start" }>
                                    <div class=move || format!(
                                        "max-w-[80%] px-3 py-2 rounded-xl text-sm {}",
                                        if is_user { "bg-orange-500 text-white" } else { "bg-gray-700 text-gray-100" }
                                    )>
                                        {msg.content.clone()}
                                    </div>
                                </div>
                            }
                        }
                    />
                    <Show when=move || is_loading.get()>
                        <div class="flex justify-start">
                            <div class="bg-gray-700 text-gray-300 px-3 py-2 rounded-xl text-sm">
                                "🦊 생각 중..."
                            </div>
                        </div>
                    </Show>
                </div>

                // 음성 시각화 (Time Domain + FFT + STT)
                <Show when=move || is_listening.get()>
                    <div class="px-3 py-2 border-t border-gray-700">
                        <AudioVisualizer 
                            is_active=is_listening 
                            transcript=interim_transcript
                            width=260 
                            height=100 
                        />
                    </div>
                </Show>

                // 입력 영역
                <div class="p-3 border-t border-gray-700 flex gap-2">
                    <button
                        class="w-10 h-10 rounded-full flex items-center justify-center transition-colors"
                        class:bg-red-500=is_listening
                        class:bg-gray-700=move || !is_listening.get() && !mic_disabled()
                        class:bg-gray-500=mic_disabled
                        class:opacity-50=mic_disabled
                        class:cursor-not-allowed=mic_disabled
                        class:animate-pulse=is_listening
                        disabled=mic_disabled
                        on:click=start_listening
                        title=move || if mic_disabled() { "TTS 재생 중..." } else { "음성 입력" }
                    >
                        {move || if is_tts_playing.get() { "🔇" } else { "🎤" }}
                    </button>
                    <input
                        type="text"
                        class="flex-1 bg-gray-800 border border-gray-600 rounded-lg px-3 text-white text-sm focus:outline-none focus:border-orange-500"
                        placeholder="샘한테 말해봐..."
                        prop:value=move || input_text.get()
                        on:input=move |e| set_input_text.set(event_target_value(&e))
                        on:keypress={
                            let send_message = send_message.clone();
                            move |e| {
                                if e.key() == "Enter" {
                                    send_message(());
                                }
                            }
                        }
                    />
                    <button
                        class="w-10 h-10 rounded-full bg-orange-500 hover:bg-orange-600 flex items-center justify-center transition-colors"
                        on:click={
                            let send_message = send_message.clone();
                            move |_| send_message(())
                        }
                    >
                        "→"
                    </button>
                </div>
            </div>
        </Show>
    }
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Clawdbot Gateway API 호출
async fn send_to_sam(message: &str) -> Result<String, String> {
    use gloo_net::http::Request;
    
    let api_url = "/api/chat";
    
    let body = serde_json::json!({
        "message": message,
        "source": "vla-emotion"
    });

    let response = Request::post(api_url)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if response.ok() {
        let json: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        Ok(json["response"].as_str().unwrap_or("응답 없음").to_string())
    } else {
        Err(format!("API 오류: {}", response.status()))
    }
}

/// TTS - 콜백과 함께 음성 합성 (에코 방지)
fn speak_with_callback(text: &str, set_is_tts_playing: WriteSignal<bool>) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(synthesis) = window.speech_synthesis() {
                // 이전 음성 중단
                let _ = synthesis.cancel();
                
                if let Ok(utterance) = SpeechSynthesisUtterance::new_with_text(text) {
                    utterance.set_lang("ko-KR");
                    utterance.set_rate(1.0);
                    utterance.set_pitch(1.0);
                    
                    // TTS 시작 전 상태 설정
                    set_is_tts_playing.set(true);
                    IS_SPEAKING.store(true, Ordering::SeqCst);
                    
                    // TTS 종료 시 콜백
                    let set_tts = set_is_tts_playing.clone();
                    let onend = Closure::wrap(Box::new(move |_: web_sys::Event| {
                        IS_SPEAKING.store(false, Ordering::SeqCst);
                        // 약간의 딜레이 후 마이크 활성화 (잔향 방지)
                        let set_tts_inner = set_tts.clone();
                        if let Some(window) = web_sys::window() {
                            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                                &Closure::once_into_js(move || {
                                    set_tts_inner.set(false);
                                }).unchecked_ref(),
                                500, // 0.5초 딜레이
                            );
                        }
                    }) as Box<dyn FnMut(web_sys::Event)>);
                    
                    utterance.set_onend(Some(onend.as_ref().unchecked_ref()));
                    onend.forget();
                    
                    let _ = synthesis.speak(&utterance);
                }
            }
        }
    }
    
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = text;
        let _ = set_is_tts_playing;
    }
}
