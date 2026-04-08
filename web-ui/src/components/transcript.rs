use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use crate::app::{TranscriptEntry, SentimentData};
use crate::components::icons;
use crate::components::audio_visualizer::AudioVisualizer;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = webkitSpeechRecognition)]
    type SpeechRecognition;
    
    #[wasm_bindgen(constructor, js_class = "webkitSpeechRecognition")]
    fn new() -> SpeechRecognition;
    
    #[wasm_bindgen(method, setter)]
    fn set_continuous(this: &SpeechRecognition, val: bool);
    
    #[wasm_bindgen(method, setter)]
    fn set_interimResults(this: &SpeechRecognition, val: bool);
    
    #[wasm_bindgen(method, setter)]
    fn set_lang(this: &SpeechRecognition, val: &str);
    
    #[wasm_bindgen(method)]
    fn start(this: &SpeechRecognition);
    
    #[wasm_bindgen(method)]
    fn stop(this: &SpeechRecognition);
    
    #[wasm_bindgen(method, setter)]
    fn set_onresult(this: &SpeechRecognition, callback: &js_sys::Function);
    
    #[wasm_bindgen(method, setter)]
    fn set_onerror(this: &SpeechRecognition, callback: &js_sys::Function);
    
    #[wasm_bindgen(method, setter)]
    fn set_onend(this: &SpeechRecognition, callback: &js_sys::Function);
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
    
    // 음성 인식 시작/종료
    Effect::new(move |_| {
        if is_active.get() {
            if let Some(cid) = case_id.get() {
                log::info!("Starting speech recognition for case: {}", cid);
                start_speech_recognition(set_transcripts, set_interim_text, set_sentiments, set_last_analysis, set_last_reasoning);
            }
        } else {
            set_interim_text.set(String::new());
            set_last_analysis.set(None);
            set_last_reasoning.set(None);
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
                    // 🎵 음성 스펙트럼 시각화 + STT
                    <AudioVisualizer 
                        is_active=is_active
                        transcript=interim_text
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
                                    
                                    // 분석 + 추론 실행
                                    let text_for_analysis = text.clone();
                                    let text_for_reasoning = text.clone();
                                    
                                    spawn_local(async move {
                                        if let Ok(analysis) = analyze_text(&text_for_analysis).await {
                                            set_last_analysis.set(Some(analysis.summary.clone()));
                                            set_sentiments.update(|s| {
                                                s.push(SentimentData {
                                                    timestamp: js_sys::Date::now(),
                                                    sentiment: analysis.sentiment,
                                                    confidence: analysis.confidence,
                                                });
                                            });
                                        }
                                    });
                                    
                                    let set_reasoning = set_last_reasoning.clone();
                                    spawn_local(async move {
                                        if let Ok(reasoning) = reason_text(&text_for_reasoning).await {
                                            set_reasoning.set(Some(reasoning));
                                        }
                                    });
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

/// Web Speech API로 음성 인식 시작
fn start_speech_recognition(
    set_transcripts: WriteSignal<Vec<TranscriptEntry>>,
    set_interim_text: WriteSignal<String>,
    set_sentiments: WriteSignal<Vec<SentimentData>>,
    set_last_analysis: WriteSignal<Option<String>>,
    set_last_reasoning: WriteSignal<Option<ReasoningResult>>,
) {
    use std::rc::Rc;
    use std::cell::RefCell;
    
    // Web Speech API 지원 확인
    let window = web_sys::window().expect("no window");
    
    // webkitSpeechRecognition 존재 확인
    let has_speech = js_sys::Reflect::has(&window, &JsValue::from_str("webkitSpeechRecognition"))
        .unwrap_or(false);
    
    if !has_speech {
        log::warn!("Web Speech API not supported in this browser");
        return;
    }
    
    let recognition = SpeechRecognition::new();
    recognition.set_continuous(true);
    recognition.set_interimResults(true);
    recognition.set_lang("ko-KR");  // 한국어
    
    // 🔧 FIX: 처리된 결과 인덱스 추적 (중복 방지)
    let processed_index = Rc::new(RefCell::new(0u32));
    let processed_index_clone = processed_index.clone();
    
    // 결과 콜백
    let on_result = Closure::wrap(Box::new(move |event: web_sys::Event| {
        // event.results 접근
        if let Ok(results) = js_sys::Reflect::get(&event, &JsValue::from_str("results")) {
            let results = js_sys::Array::from(&results);
            let len = results.length();
            
            // 🔧 FIX: 모든 결과 순회 (마지막만 아니라)
            let start_idx = *processed_index_clone.borrow();
            
            for i in start_idx..len {
                let result = results.get(i);
                
                // transcript 추출
                if let Ok(first) = js_sys::Reflect::get(&result, &JsValue::from_str("0")) {
                    if let Ok(transcript) = js_sys::Reflect::get(&first, &JsValue::from_str("transcript")) {
                        if let Some(text) = transcript.as_string() {
                            let text = text.trim().to_string();
                            
                            if let Ok(is_final) = js_sys::Reflect::get(&result, &JsValue::from_str("isFinal")) {
                                if is_final.as_bool().unwrap_or(false) {
                                    // 최종 결과 - 처리된 인덱스 업데이트
                                    *processed_index_clone.borrow_mut() = i + 1;
                                    
                                    if !text.is_empty() {
                                        log::info!("Final [{}]: {}", i, text);
                                        let text_clone = text.clone();
                                        set_transcripts.update(|entries| {
                                            entries.push(TranscriptEntry {
                                                timestamp: js_sys::Date::now(),
                                                text: text_clone,
                                                speaker: "client".to_string(),
                                            });
                                        });
                                        set_interim_text.set(String::new());
                                        
                                        // 🔍 문장 분석 + 🧠 Neurosymbolic 추론
                                        let text_for_analysis = text.clone();
                                        let text_for_reasoning = text.clone();
                                        
                                        // 감정 분석
                                        spawn_local(async move {
                                            if let Ok(analysis) = analyze_text(&text_for_analysis).await {
                                                log::info!("Analysis: {}", analysis.summary);
                                                set_last_analysis.set(Some(analysis.summary.clone()));
                                                set_sentiments.update(|s| {
                                                    s.push(SentimentData {
                                                        timestamp: js_sys::Date::now(),
                                                        sentiment: analysis.sentiment,
                                                        confidence: analysis.confidence,
                                                    });
                                                });
                                            }
                                        });
                                        
                                        // Neurosymbolic 추론
                                        let set_reasoning = set_last_reasoning.clone();
                                        spawn_local(async move {
                                            if let Ok(reasoning) = reason_text(&text_for_reasoning).await {
                                                log::info!("Reasoning: {}", reasoning.summary);
                                                set_reasoning.set(Some(reasoning));
                                            }
                                        });
                                    }
                                } else if i == len - 1 {
                                    // interim 결과 (마지막 것만 표시)
                                    log::debug!("Interim: {}", text);
                                    set_interim_text.set(text);
                                }
                            }
                        }
                    }
                }
            }
        }
    }) as Box<dyn FnMut(web_sys::Event)>);
    
    recognition.set_onresult(on_result.as_ref().unchecked_ref());
    on_result.forget();
    
    // 에러 콜백
    let on_error = Closure::wrap(Box::new(move |event: web_sys::Event| {
        if let Ok(error) = js_sys::Reflect::get(&event, &JsValue::from_str("error")) {
            log::error!("Speech recognition error: {:?}", error);
        }
    }) as Box<dyn FnMut(web_sys::Event)>);
    
    recognition.set_onerror(on_error.as_ref().unchecked_ref());
    on_error.forget();
    
    // 종료 시 자동 재시작 (continuous 모드 유지)
    let set_transcripts_clone = set_transcripts.clone();
    let set_interim_clone = set_interim_text.clone();
    let set_sentiments_clone = set_sentiments.clone();
    let set_analysis_clone = set_last_analysis.clone();
    let set_reasoning_clone = set_last_reasoning.clone();
    
    let on_end = Closure::wrap(Box::new(move |_: web_sys::Event| {
        // 🔧 FIX: 재시작 갭 축소 (500ms → 100ms)
        log::info!("Speech recognition ended, restarting in 100ms...");
        
        let set_tr = set_transcripts_clone.clone();
        let set_int = set_interim_clone.clone();
        let set_sent = set_sentiments_clone.clone();
        let set_anal = set_analysis_clone.clone();
        let set_reas = set_reasoning_clone.clone();
        
        let window = web_sys::window().unwrap();
        let closure = Closure::once(Box::new(move || {
            log::info!("Restarting speech recognition...");
            start_speech_recognition(set_tr, set_int, set_sent, set_anal, set_reas);
        }) as Box<dyn FnOnce()>);
        
        // 🔧 FIX: 100ms로 단축 (기존 500ms)
        window.set_timeout_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            100
        ).unwrap();
        closure.forget();
    }) as Box<dyn FnMut(web_sys::Event)>);
    
    recognition.set_onend(on_end.as_ref().unchecked_ref());
    on_end.forget();
    
    // 시작!
    recognition.start();
    log::info!("Speech recognition started (tracking from index 0)");
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
#[derive(Clone, Debug, serde::Deserialize)]
pub struct ReasoningResult {
    pub input: String,
    pub entities: Vec<EntityInfo>,
    pub relations: Vec<RelationInfo>,
    pub solutions: Vec<SolutionInfo>,
    pub critical: bool,
    pub priority: String,
    pub summary: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct EntityInfo {
    pub text: String,
    #[serde(rename = "type")]
    pub entity_type: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct RelationInfo {
    pub name: String,
    pub subject: String,
    pub object: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
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
