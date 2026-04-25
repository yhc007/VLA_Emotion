use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use crate::components::{
    video_capture::VideoCapture,
    sentiment_display::SentimentDisplay,
    transcript::{Transcript, ReasoningResult},
    report_viewer::ReportViewer,
    session_control::SessionControl,
    document_upload::{DocumentUpload, UploadedDoc},
    timeline::{Timeline, TimelineMarker, MarkerType},
    speaker_diarization::SpeakerDiarization,
    saved_sessions::{SavedSessions, LoadedSessionResult},
    icons,
    sam_chat::SamChat,
    reasoning_graph::ReasoningGraph,
};
use crate::google::{GoogleSheets, GoogleDrive, auth::GoogleLoginButton, drive::format_session_markdown};

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

    // 영상 분석 활성화 토글 (OFF일 때 음성만 분석)
    let (video_enabled, set_video_enabled) = signal(true);
    
    // Neurosymbolic 추론 결과
    let (reasoning_result, set_reasoning_result) = signal(Option::<ReasoningResult>::None);
    
    // 화자 분리 상태
    let (current_speaker, set_current_speaker) = signal(Option::<String>::None);
    let (audio_stream, set_audio_stream) = signal(Option::<web_sys::MediaStream>::None);
    
    // MediaSet 상태
    let (documents, set_documents) = signal(Vec::<UploadedDoc>::new());
    let (timeline_markers, set_timeline_markers) = signal(Vec::<TimelineMarker>::new());

    // 저장된 세션 불러오기 상태
    let (show_saved_sessions, set_show_saved_sessions) = signal(false);
    let (loaded_session, set_loaded_session) = signal(Option::<LoadedSessionResult>::None);
    let (is_replay_mode, set_is_replay_mode) = signal(false);
    
    // 타임라인 시간 추적
    let (start_time, set_start_time) = signal(0.0_f64);
    let (current_ms, set_current_ms) = signal(0_u64);
    let (duration_ms, set_duration_ms) = signal(0_u64);
    
    // ── 저장된 세션 불러오기 Effect ──
    Effect::new(move |_| {
        if let Some(loaded) = loaded_session.get() {
            // 1. 패널 닫기
            set_show_saved_sessions.set(false);

            // 2. 기존 케이스 활성 해제
            set_is_active.set(false);
            set_is_replay_mode.set(true);

            // 3. 케이스 ID 설정
            set_case_id.set(Some(loaded.session_id.clone()));

            // 4. 대화 기록 로드
            set_transcripts.set(loaded.transcripts.clone());

            // 5. 감정 데이터 로드
            set_sentiments.set(loaded.sentiments.clone());

            // 6. 추론 결과 로드
            if let Some(reasoning) = &loaded.reasoning {
                set_reasoning_result.set(Some(reasoning.clone()));
            }

            // 7. 타임라인 마커 로드
            set_timeline_markers.set(loaded.timeline_markers.clone());

            // 8. 시간 설정
            set_duration_ms.set(loaded.duration_ms);
            set_current_ms.set(loaded.duration_ms); // 끝까지 표시

            log::info!("Session loaded: {} ({} transcripts, {} sentiments)",
                loaded.session_id,
                loaded.transcripts.len(),
                loaded.sentiments.len(),
            );
        }
    });

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
    
    // ── Google 연동 상태 ──
    // TODO: Google Cloud Console에서 OAuth2 Client ID를 발급받아 교체하세요
    let google_client_id = "712685283607-jitjm9avh4dk087umvir8o2r8rg91mh0.apps.googleusercontent.com".to_string();
    let (google_logged_in, set_google_logged_in) = signal(false);
    let (google_token, set_google_token) = signal(Option::<String>::None);
    let (google_sheet_id, set_google_sheet_id) = signal(Option::<String>::None);
    let (google_sheet_url, set_google_sheet_url) = signal(Option::<String>::None);
    let (google_folder_id, set_google_folder_id) = signal(Option::<String>::None);
    let (google_sync_status, set_google_sync_status) = signal("idle".to_string());

    // Google 로그인 콜백
    let on_google_login = Callback::new(move |token: String| {
        set_google_token.set(Some(token.clone()));
        set_google_logged_in.set(true);
        log::info!("Google auth token set");

        // Drive 폴더 초기화
        spawn_local(async move {
            let drive = GoogleDrive::new(&token);
            match drive.get_or_create_vla_folder().await {
                Ok(folder_id) => {
                    set_google_folder_id.set(Some(folder_id));
                    log::info!("Google Drive VLA folder ready");
                }
                Err(e) => log::error!("Failed to init Drive folder: {}", e),
            }
        });
    });

    // 케이스 시작 시 Google Sheet 자동 생성
    Effect::new(move |_| {
        if is_active.get() {
            if let (Some(cid), Some(token)) = (case_id.get(), google_token.get()) {
                let date = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
                let date_short = date.split('T').next().unwrap_or(&date).to_string();
                set_google_sync_status.set("creating sheet...".to_string());

                spawn_local(async move {
                    let sheets = GoogleSheets::new(&token);
                    match sheets.create_session_spreadsheet(&cid, &date_short).await {
                        Ok((sid, url)) => {
                            log::info!("Google Sheet created: {}", url);
                            set_google_sheet_id.set(Some(sid.clone()));
                            set_google_sheet_url.set(Some(url));
                            set_google_sync_status.set("syncing".to_string());

                            // 시트를 VLA 폴더로 이동
                            if let Some(folder_id) = google_folder_id.get_untracked() {
                                let drive = GoogleDrive::new(&token);
                                if let Err(e) = drive.move_to_folder(&sid, &folder_id).await {
                                    log::warn!("Failed to move sheet to folder: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to create sheet: {}", e);
                            set_google_sync_status.set(format!("error: {}", e));
                        }
                    }
                });
            }
        }
    });

    // 트랜스크립트 추가 시 Google Sheets 실시간 동기화
    Effect::new(move |prev_count: Option<usize>| {
        let entries = transcripts.get();
        let count = entries.len();

        if let Some(prev) = prev_count {
            if count > prev {
                if let (Some(sheet_id), Some(token)) = (google_sheet_id.get(), google_token.get()) {
                    let new_entries: Vec<_> = entries[prev..].to_vec();
                    spawn_local(async move {
                        let sheets = GoogleSheets::new(&token);
                        for entry in &new_entries {
                            let ts = {
                                let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(entry.timestamp));
                                d.to_locale_time_string("ko-KR").as_string().unwrap_or_default()
                            };
                            if let Err(e) = sheets.append_transcript(&sheet_id, &ts, &entry.speaker, &entry.text).await {
                                log::warn!("Sheets sync error: {}", e);
                            }
                        }
                    });
                }
            }
        }

        count
    });

    // 감정 데이터 추가 시 Google Sheets 동기화
    Effect::new(move |prev_count: Option<usize>| {
        let sents = sentiments.get();
        let count = sents.len();

        if let Some(prev) = prev_count {
            if count > prev {
                if let (Some(sheet_id), Some(token)) = (google_sheet_id.get(), google_token.get()) {
                    let new_sents: Vec<_> = sents[prev..].to_vec();
                    spawn_local(async move {
                        let sheets = GoogleSheets::new(&token);
                        for s in &new_sents {
                            let ts = {
                                let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(s.timestamp));
                                d.to_locale_time_string("ko-KR").as_string().unwrap_or_default()
                            };
                            if let Err(e) = sheets.append_sentiment(&sheet_id, &ts, &s.sentiment, s.confidence, "").await {
                                log::warn!("Sheets sentiment sync error: {}", e);
                            }
                        }
                    });
                }
            }
        }

        count
    });

    // 케이스 종료 시 Google Drive에 리포트 업로드
    Effect::new(move |was_active: Option<bool>| {
        let active = is_active.get();

        // active → inactive 전환 감지
        if was_active == Some(true) && !active {
            if let (Some(token), Some(folder_id), Some(cid)) =
                (google_token.get(), google_folder_id.get(), case_id.get())
            {
                let transcripts_data: Vec<_> = transcripts.get_untracked().iter().map(|t| {
                    let ts = {
                        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(t.timestamp));
                        d.to_locale_time_string("ko-KR").as_string().unwrap_or_default()
                    };
                    (ts, t.speaker.clone(), t.text.clone())
                }).collect();

                let sentiments_data: Vec<_> = sentiments.get_untracked().iter().map(|s| {
                    let ts = {
                        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(s.timestamp));
                        d.to_locale_time_string("ko-KR").as_string().unwrap_or_default()
                    };
                    (ts, s.sentiment.clone(), s.confidence)
                }).collect();

                let date = js_sys::Date::new_0().to_iso_string().as_string().unwrap_or_default();
                let date_short = date.split('T').next().unwrap_or(&date).to_string();

                set_google_sync_status.set("uploading report...".to_string());
                spawn_local(async move {
                    let drive = GoogleDrive::new(&token);

                    // 마크다운 리포트 업로드
                    let md = format_session_markdown(
                        &cid,
                        &date_short,
                        &transcripts_data,
                        &sentiments_data,
                        None,
                    );

                    match drive.upload_session_summary(&folder_id, &cid, &date_short, &md).await {
                        Ok((_, url)) => {
                            log::info!("Report uploaded: {}", url);
                            set_google_sync_status.set("saved".to_string());
                        }
                        Err(e) => {
                            log::error!("Report upload failed: {}", e);
                            set_google_sync_status.set(format!("error: {}", e));
                        }
                    }
                });
            }
        }

        active
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
                    <div class="flex items-center gap-3 text-xs text-grok-muted font-mono">
                        // Google 로그인 + 동기화 상태
                        <GoogleLoginButton
                            client_id=google_client_id.clone()
                            on_login=on_google_login
                            is_logged_in=google_logged_in
                        />
                        <Show when=move || google_logged_in.get()>
                            {move || {
                                let status = google_sync_status.get();
                                let (color, icon) = match status.as_str() {
                                    "syncing" => ("text-green-400", "~"),
                                    "saved" => ("text-green-400", "v"),
                                    "idle" => ("text-gray-500", "-"),
                                    s if s.starts_with("error") => ("text-red-400", "!"),
                                    _ => ("text-yellow-400", "*"),
                                };
                                view! {
                                    <span class=format!("text-[10px] {}", color)>
                                        {format!("[{}] {}", icon, status)}
                                    </span>
                                }
                            }}
                        </Show>
                        // Google Sheets 링크
                        <Show when=move || google_sheet_url.get().is_some()>
                            <a
                                href=move || google_sheet_url.get().unwrap_or_default()
                                target="_blank"
                                class="text-[10px] text-grok-blue hover:underline"
                            >"Sheet"</a>
                        </Show>

                        // 리플레이 모드 표시
                        <Show when=move || is_replay_mode.get()>
                            <span class="px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-400 text-xs font-medium border border-amber-500/30">
                                "REPLAY"
                            </span>
                        </Show>
                        <div class="w-1.5 h-1.5 rounded-full bg-green-500" />
                        "System Online"
                        // 경과 시간 표시
                        <Show when=move || is_active.get() || is_replay_mode.get()>
                            <span class="ml-1 text-grok-blue">
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
                
                // Case 컨트롤 + 녹음 불러오기
                <div class="flex items-center gap-3">
                    <div class="flex-1">
                        <SessionControl
                            case_id=case_id
                            is_active=is_active
                            set_case_id=set_case_id
                            set_is_active=set_is_active
                            set_show_report=set_show_report
                        />
                    </div>
                    // 저장된 녹음 불러오기 버튼
                    <Show when=move || !is_active.get()>
                        <button
                            on:click=move |_| set_show_saved_sessions.set(true)
                            class="grok-btn flex items-center gap-2 px-4 py-2.5 bg-grok-gray hover:bg-grok-border text-white text-sm font-medium rounded-md border border-grok-border transition-colors"
                            title="Load saved recording"
                        >
                            {icons::folder_open()}
                            <span class="hidden sm:inline">"Load Recording"</span>
                        </button>
                    </Show>
                </div>
                
                // 메인 그리드 (3컬럼)
                <div class="grid grid-cols-1 lg:grid-cols-3 gap-4 mt-4">
                    // 왼쪽: 비디오 + 센티먼트
                    <div class="space-y-4">
                        <VideoCapture
                            is_active=is_active
                            case_id=case_id
                            set_sentiments=set_sentiments
                            video_enabled=video_enabled
                            set_video_enabled=set_video_enabled
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
                    {
                        let on_seek: Option<Box<dyn Fn(u64)>> = None;
                        view! {
                            <Timeline
                                markers=timeline_markers
                                duration_ms=duration_ms
                                current_ms=current_ms
                                on_seek=on_seek
                            />
                        }
                    }
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

                // 저장된 세션 불러오기 모달
                <Show when=move || show_saved_sessions.get()>
                    <SavedSessions
                        on_load=set_loaded_session
                        on_close=Callback::new(move |_| set_show_saved_sessions.set(false))
                    />
                </Show>
            </div>
            
            // 🦊 샘 채팅 버튼 (플로팅)
            <SamChat />
        </main>
    }
}
