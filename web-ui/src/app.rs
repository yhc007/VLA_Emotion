use leptos::prelude::*;
use crate::components::{
    video_capture::VideoCapture,
    sentiment_display::SentimentDisplay,
    transcript::Transcript,
    report_viewer::ReportViewer,
    session_control::SessionControl,
    icons,
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
    
    view! {
        <main class="min-h-screen bg-grok-black">
            <div class="max-w-6xl mx-auto px-6 py-8">
                // 헤더 - Palantir 스타일
                <header class="mb-8 flex items-center justify-between">
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
                
                // 메인 그리드
                <div class="grid grid-cols-1 lg:grid-cols-2 gap-4 mt-4">
                    // 왼쪽: 비디오 + 센티먼트
                    <div class="space-y-4">
                        <VideoCapture
                            is_active=is_active
                            case_id=case_id
                            set_sentiments=set_sentiments
                        />
                        <SentimentDisplay sentiments=sentiments />
                    </div>
                    
                    // 오른쪽: 대화 기록
                    <div class="h-full">
                        <Transcript
                            is_active=is_active
                            case_id=case_id
                            transcripts=transcripts
                            set_transcripts=set_transcripts
                        />
                    </div>
                </div>
                
                // 푸터
                <footer class="mt-12 pt-4 border-t border-grok-border">
                    <div class="flex items-center justify-between text-xs text-grok-muted font-mono">
                        <span>"VLA v0.1.0"</span>
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
        </main>
    }
}
