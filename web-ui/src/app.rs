use leptos::prelude::*;
use crate::components::{
    video_capture::VideoCapture,
    sentiment_display::SentimentDisplay,
    transcript::{Transcript, ReasoningResult},
    report_viewer::ReportViewer,
    session_control::SessionControl,
    document_upload::{DocumentUpload, UploadedDoc},
    timeline::{Timeline, TimelineMarker, MarkerType},
    speaker_diarization::SpeakerDiarization,
    icons,
    sam_chat::SamChat,
    reasoning_graph::ReasoningGraph,
};

/// Case 상태
#[derive(Clone, Debug, Default)]
pub struct CaseState {
    pub case_id: Option<String>,
    pub is_active: bool,
    pub sentiments: Vec<SentimentData>,
    pub transcripts: Vec<TranscriptEntry>,
}

#[derive(Clone, Debug)]
pub struct SentimentData {
    pub timestamp: f64,
    pub sentiment: String,
    pub confidence: f64,
}

#[derive(Clone, Debug)]
pub struct TranscriptEntry {
    pub timestamp: f64,
    pub text: String,
    pub speaker: String,
}

#[component]
pub fn App() -> impl IntoView {
    // Case 상태
    let (case_id, set_case_id) = signal(Option::<String>::None);
    let (is_active, set_is_active) = signal(false);
    let (sentiments, set_sentiments) = signal(Vec::<SentimentData>::new());
    let (transcripts, set_transcripts) = signal(Vec::<TranscriptEntry>::new());
    let (show_report, set_show_report) = signal(false);
    
    // Neurosymbolic 추론 결과
    let (reasoning_result, set_reasoning_result) = signal(Option::<ReasoningResult>::None);
    
    // 화자 분리 상태
    let (current_speaker, set_current_speaker) = signal(Option::<String>::None);
    let (audio_stream, set_audio_stream) = signal(Option::<web_sys::MediaStream>::None);
    
    // MediaSet 상태
    let (documents, set_documents) = signal(Vec::<UploadedDoc>::new());
    let (timeline_markers, set_timeline_markers) = signal(Vec::<TimelineMarker>::new());
    
    // 타임라인 시간 추적
    let (start_time, set_start_time) = signal(0.0_f64);
    let (current_ms, set_current_ms) = signal(0_u64);
    let (duration_ms, set_duration_ms) = signal(0_u64);
    
    // 케이스 시작 시 타임라인 초기화
    Effect::new(move |_| {
        if is_active.get() {
            let now = js_sys::Date::now();
            set_start_time.set(now);
            set_current_ms.set(0);
            set_duration_ms.set(0);
            
            // 시작 마커 추가
            set_timeline_markers.update(|markers| {
                markers.clear(); // 이전 마커 클리어
                markers.push(TimelineMarker {
                    id: format!("start-{}", now as u64),
                    timestamp_ms: 0,
                    marker_type: MarkerType::Start,
                    label: "Case Started".to_string(),
                    description: None,
                });
            });
        } else {
            // 케이스 종료 시
            let st = start_time.get();
            if st > 0.0 {
                let end_time = js_sys::Date::now();
                let elapsed = (end_time - st) as u64;
                
                set_timeline_markers.update(|markers| {
                    markers.push(TimelineMarker {
                        id: format!("end-{}", end_time as u64),
                        timestamp_ms: elapsed,
                        marker_type: MarkerType::End,
                        label: "Case Ended".to_string(),
                        description: None,
                    });
                });
            }
        }
    });
    
    // 타이머: 매초 current_ms 업데이트
    Effect::new(move |_| {
        if is_active.get() {
            let handle = gloo_timers::callback::Interval::new(1000, move || {
                if is_active.get() {
                    let st = start_time.get();
                    if st > 0.0 {
                        let now = js_sys::Date::now();
                        let elapsed = (now - st) as u64;
                        set_current_ms.set(elapsed);
                        set_duration_ms.set(elapsed); // 실시간으로 duration도 업데이트
                    }
                }
            });
            handle.forget();
        }
    });
    
    // 감정 변화 감지 시 마커 추가
    Effect::new(move |prev_sentiment: Option<String>| {
        let sents = sentiments.get();
        if let Some(latest) = sents.last() {
            let current_sentiment = latest.sentiment.clone();
            
            // 이전 감정과 다르면 마커 추가
            if prev_sentiment.as_ref() != Some(&current_sentiment) && is_active.get() {
                let st = start_time.get();
                if st > 0.0 {
                    let elapsed = (js_sys::Date::now() - st) as u64;
                    set_timeline_markers.update(|markers| {
                        markers.push(TimelineMarker {
                            id: format!("emotion-{}", js_sys::Date::now() as u64),
                            timestamp_ms: elapsed,
                            marker_type: MarkerType::SentimentChange,
                            label: format!("Emotion: {}", current_sentiment),
                            description: None,
                        });
                    });
                }
            }
            
            return current_sentiment;
        }
        prev_sentiment.unwrap_or_default()
    });
    
    // 문서 업로드 시 마커 추가
    Effect::new(move |prev_count: Option<usize>| {
        let docs = documents.get();
        let count = docs.len();
        
        if let Some(prev) = prev_count {
            if count > prev && is_active.get() {
                let st = start_time.get();
                if st > 0.0 {
                    let elapsed = (js_sys::Date::now() - st) as u64;
                    if let Some(doc) = docs.last() {
                        set_timeline_markers.update(|markers| {
                            markers.push(TimelineMarker {
                                id: format!("doc-{}", js_sys::Date::now() as u64),
                                timestamp_ms: elapsed,
                                marker_type: MarkerType::DocumentUpload,
                                label: format!("Document: {}", doc.filename),
                                description: None,
                            });
                        });
                    }
                }
            }
        }
        
        count
    });
    
    // 추론 결과 변화 시 마커 추가
    Effect::new(move |_| {
        if let Some(result) = reasoning_result.get() {
            if is_active.get() {
                let st = start_time.get();
                if st > 0.0 {
                    let elapsed = (js_sys::Date::now() - st) as u64;
                    
                    // 문제 발견 마커
                    if !result.entities.is_empty() {
                        set_timeline_markers.update(|markers| {
                            // 중복 방지
                            let exists = markers.iter().any(|m| 
                                m.marker_type == MarkerType::ProblemFound && 
                                (elapsed as i64 - m.timestamp_ms as i64).abs() < 2000
                            );
                            if !exists {
                                markers.push(TimelineMarker {
                                    id: format!("insight-{}", js_sys::Date::now() as u64),
                                    timestamp_ms: elapsed,
                                    marker_type: MarkerType::Insight,
                                    label: format!("{} entities found", result.entities.len()),
                                    description: None,
                                });
                            }
                        });
                    }
                    
                    // 솔루션 제안 마커
                    if !result.solutions.is_empty() {
                        set_timeline_markers.update(|markers| {
                            let exists = markers.iter().any(|m| 
                                m.marker_type == MarkerType::SolutionProposed && 
                                (elapsed as i64 - m.timestamp_ms as i64).abs() < 2000
                            );
                            if !exists {
                                markers.push(TimelineMarker {
                                    id: format!("solution-{}", js_sys::Date::now() as u64),
                                    timestamp_ms: elapsed,
                                    marker_type: MarkerType::SolutionProposed,
                                    label: format!("{} solutions", result.solutions.len()),
                                    description: None,
                                });
                            }
                        });
                    }
                }
            }
        }
    });
    
    // 화자 변경 시 타임라인 마커 추가
    Effect::new(move |prev_speaker: Option<String>| {
        if let Some(speaker) = current_speaker.get() {
            if is_active.get() && prev_speaker.as_ref() != Some(&speaker) {
                let st = start_time.get();
                if st > 0.0 {
                    let elapsed = (js_sys::Date::now() - st) as u64;
                    set_timeline_markers.update(|markers| {
                        markers.push(TimelineMarker {
                            id: format!("speaker-{}", js_sys::Date::now() as u64),
                            timestamp_ms: elapsed,
                            marker_type: MarkerType::SpeakerChange,
                            label: format!("Speaker: {}", speaker),
                            description: None,
                        });
                    });
                }
            }
            speaker
        } else {
            prev_speaker.unwrap_or_default()
        }
    });
    
    // 대화 내용 추가 시 마커 생성 (화자 발화 기록)
    Effect::new(move |prev_count: Option<usize>| {
        let entries = transcripts.get();
        let count = entries.len();
        
        if let Some(prev) = prev_count {
            if count > prev && is_active.get() {
                let st = start_time.get();
                if st > 0.0 {
                    // 새로 추가된 대화들 처리
                    for entry in entries.iter().skip(prev) {
                        let elapsed = (entry.timestamp - st) as u64;
                        let speaker_label = match entry.speaker.as_str() {
                            "client" => "Client",
                            "analyst" | "counselor" => "Analyst",
                            s if s.starts_with("speaker_") => {
                                match s {
                                    "speaker_low" => "Speaker A",
                                    "speaker_mid" => "Speaker B",
                                    "speaker_high" => "Speaker C",
                                    _ => "Speaker"
                                }
                            }
                            _ => &entry.speaker
                        };
                        
                        set_timeline_markers.update(|markers| {
                            markers.push(TimelineMarker {
                                id: format!("speech-{}", entry.timestamp as u64),
                                timestamp_ms: elapsed,
                                marker_type: MarkerType::SpeakerChange,
                                label: speaker_label.to_string(),
                                description: Some(entry.text.clone()),
                            });
                        });
                    }
                }
            }
        }
        
        count
    });
    
    view! {
        <main class="min-h-screen bg-grok-black">
            <div class="max-w-7xl mx-auto px-6 py-6">
                // 헤더
                <header class="mb-6 flex items-center justify-between">
                    <div class="flex items-center gap-3">
                        <div class="text-grok-blue">
                            {icons::target()}
                        </div>
                        <div>
                            <h1 class="text-lg font-semibold text-white tracking-tight">
                                "VLA"
                            </h1>
                            <p class="text-xs text-grok-muted font-mono uppercase tracking-wider">
                                "Problem Discovery & Solution"
                            </p>
                        </div>
                    </div>
                    <div class="flex items-center gap-2 text-xs text-grok-muted font-mono">
                        <div class="w-1.5 h-1.5 rounded-full bg-green-500" />
                        "System Online"
                        // 경과 시간 표시
                        <Show when=move || is_active.get()>
                            <span class="ml-2 text-grok-blue">
                                {move || {
                                    let ms = current_ms.get();
                                    let secs = ms / 1000;
                                    let mins = secs / 60;
                                    let s = secs % 60;
                                    format!("{}:{:02}", mins, s)
                                }}
                            </span>
                        </Show>
                    </div>
                </header>
                
                // Case 컨트롤
                <SessionControl
                    case_id=case_id
                    is_active=is_active
                    set_case_id=set_case_id
                    set_is_active=set_is_active
                    set_show_report=set_show_report
                />
                
                // 메인 그리드 (3컬럼)
                <div class="grid grid-cols-1 lg:grid-cols-3 gap-4 mt-4">
                    // 왼쪽: 비디오 + 센티먼트
                    <div class="space-y-4">
                        <VideoCapture
                            is_active=is_active
                            case_id=case_id
                            set_sentiments=set_sentiments
                        />
                        <SentimentDisplay sentiments=sentiments />
                    </div>
                    
                    // 중앙: 대화 기록 + 화자 분리
                    <div class="h-full space-y-4">
                        // 화자 분리 패널
                        <SpeakerDiarization
                            audio_stream=audio_stream
                            is_active=is_active
                            on_speaker_change=set_current_speaker
                        />
                        
                        // 대화 기록
                        <Transcript
                            is_active=is_active
                            case_id=case_id
                            transcripts=transcripts
                            set_transcripts=set_transcripts
                            set_sentiments=set_sentiments
                            set_reasoning_result=set_reasoning_result
                            current_speaker=current_speaker
                        />
                    </div>
                    
                    // 오른쪽: 추론 그래프 + 문서
                    <div class="space-y-4">
                        <ReasoningGraph result=reasoning_result />
                        <DocumentUpload
                            case_id=case_id
                            documents=documents
                            set_documents=set_documents
                        />
                    </div>
                </div>
                
                // 타임라인 (전체 너비)
                <div class="mt-4">
                    <Timeline
                        markers=timeline_markers
                        duration_ms=duration_ms
                        current_ms=current_ms
                        on_seek=None
                    />
                </div>
                
                // 푸터
                <footer class="mt-8 pt-4 border-t border-grok-border">
                    <div class="flex items-center justify-between text-xs text-grok-muted font-mono">
                        <span>"VLA v0.2.0"</span>
                        <span>"rust • leptos • claude"</span>
                    </div>
                </footer>
                
                // 리포트 모달
                <Show when=move || show_report.get()>
                    <ReportViewer
                        case_id=case_id
                        on_close=move |_| set_show_report.set(false)
                    />
                </Show>
            </div>
            
            // 🦊 샘 채팅 버튼 (플로팅)
            <SamChat />
        </main>
    }
}
