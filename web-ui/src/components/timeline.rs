use leptos::prelude::*;
use crate::components::icons;

#[derive(Clone, Debug)]
pub struct TimelineMarker {
    pub id: String,
    pub timestamp_ms: u64,
    pub marker_type: MarkerType,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarkerType {
    Start,
    DocumentUpload,
    Insight,
    SentimentChange,
    SpeakerChange,
    ProblemFound,
    SolutionProposed,
    End,
}

impl MarkerType {
    pub fn color(&self) -> &'static str {
        match self {
            MarkerType::Start => "bg-grok-blue",
            MarkerType::DocumentUpload => "bg-purple-500",
            MarkerType::Insight => "bg-yellow-500",
            MarkerType::SentimentChange => "bg-orange-500",
            MarkerType::SpeakerChange => "bg-cyan-500",
            MarkerType::ProblemFound => "bg-red-500",
            MarkerType::SolutionProposed => "bg-green-500",
            MarkerType::End => "bg-grok-muted",
        }
    }
    
    pub fn border_color(&self) -> &'static str {
        match self {
            MarkerType::Start => "border-grok-blue",
            MarkerType::DocumentUpload => "border-purple-500",
            MarkerType::Insight => "border-yellow-500",
            MarkerType::SentimentChange => "border-orange-500",
            MarkerType::SpeakerChange => "border-cyan-500",
            MarkerType::ProblemFound => "border-red-500",
            MarkerType::SolutionProposed => "border-green-500",
            MarkerType::End => "border-grok-muted",
        }
    }
    
    pub fn text_color(&self) -> &'static str {
        match self {
            MarkerType::Start => "text-grok-blue",
            MarkerType::DocumentUpload => "text-purple-400",
            MarkerType::Insight => "text-yellow-400",
            MarkerType::SentimentChange => "text-orange-400",
            MarkerType::SpeakerChange => "text-cyan-400",
            MarkerType::ProblemFound => "text-red-400",
            MarkerType::SolutionProposed => "text-green-400",
            MarkerType::End => "text-grok-muted",
        }
    }
    
    pub fn icon(&self) -> &'static str {
        match self {
            MarkerType::Start => "▶️",
            MarkerType::DocumentUpload => "📄",
            MarkerType::Insight => "💡",
            MarkerType::SentimentChange => "🎭",
            MarkerType::SpeakerChange => "🗣️",
            MarkerType::ProblemFound => "⚠️",
            MarkerType::SolutionProposed => "✅",
            MarkerType::End => "⏹️",
        }
    }
    
    pub fn type_label(&self) -> &'static str {
        match self {
            MarkerType::Start => "시작",
            MarkerType::DocumentUpload => "문서",
            MarkerType::Insight => "인사이트",
            MarkerType::SentimentChange => "감정변화",
            MarkerType::SpeakerChange => "발화",
            MarkerType::ProblemFound => "문제발견",
            MarkerType::SolutionProposed => "해결제안",
            MarkerType::End => "종료",
        }
    }
}

#[component]
pub fn Timeline(
    markers: ReadSignal<Vec<TimelineMarker>>,
    duration_ms: ReadSignal<u64>,
    current_ms: ReadSignal<u64>,
    #[allow(unused)]
    on_seek: Option<Box<dyn Fn(u64) + 'static>>,
) -> impl IntoView {
    // 00:15 형식으로 표시
    let format_time = |ms: u64| -> String {
        let total_secs = ms / 1000;
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    };
    
    view! {
        <div class="grok-card rounded-md p-4">
            // 헤더 + 경과시간
            <div class="flex items-center justify-between mb-3">
                <h2 class="text-xs font-medium text-grok-muted uppercase tracking-wider flex items-center gap-2">
                    {icons::activity()}
                    "Timeline"
                </h2>
                <div class="flex items-center gap-2 text-xs font-mono">
                    <span class="text-grok-blue">{move || format_time(current_ms.get())}</span>
                    <span class="text-grok-muted">"/"</span>
                    <span class="text-grok-muted">{move || format_time(duration_ms.get())}</span>
                </div>
            </div>
            
            // 상단 프로그레스 바 (미니)
            <div class="h-1 bg-grok-border rounded-full overflow-hidden mb-4">
                <div 
                    class="h-full bg-gradient-to-r from-grok-blue to-cyan-500 transition-all duration-100"
                    style=move || {
                        let dur = duration_ms.get();
                        let cur = current_ms.get();
                        let pct = if dur > 0 { (cur as f64 / dur as f64) * 100.0 } else { 0.0 };
                        format!("width: {}%", pct)
                    }
                />
            </div>
            
            // 세로 타임라인
            <div class="relative max-h-[400px] overflow-y-auto pr-2">
                {move || {
                    let marks = markers.get();
                    
                    if marks.is_empty() {
                        view! {
                            <div class="flex flex-col items-center justify-center py-8 text-grok-muted">
                                <span class="text-2xl mb-2">"⏱️"</span>
                                <p class="text-xs font-mono">"이벤트 대기 중..."</p>
                            </div>
                        }.into_any()
                    } else {
                        let total_count = marks.len();
                        view! {
                            <div class="relative">
                                // 세로 연결선
                                <div class="absolute left-4 top-0 bottom-0 w-0.5 bg-grok-border"></div>
                                
                                // 마커 목록
                                <div class="space-y-3">
                                    {marks.into_iter().enumerate().map(|(idx, marker)| {
                                        let icon = marker.marker_type.icon();
                                        let color = marker.marker_type.color();
                                        let border_color = marker.marker_type.border_color();
                                        let text_color = marker.marker_type.text_color();
                                        let type_label = marker.marker_type.type_label();
                                        let time = format_time(marker.timestamp_ms);
                                        let has_desc = marker.description.is_some();
                                        let desc = marker.description.clone().unwrap_or_default();
                                        let is_latest = idx == total_count - 1;
                                        
                                        view! {
                                            <div class=format!(
                                                "relative flex items-start gap-3 pl-1 {}",
                                                if is_latest { "animate-pulse" } else { "" }
                                            )>
                                                // 아이콘 마커
                                                <div class=format!(
                                                    "relative z-10 w-7 h-7 rounded-full {} border-2 {} flex items-center justify-center text-sm flex-shrink-0",
                                                    if is_latest { color } else { "bg-grok-gray" },
                                                    border_color
                                                )>
                                                    <span>{icon}</span>
                                                </div>
                                                
                                                // 내용
                                                <div class="flex-1 min-w-0 pb-2">
                                                    // 상단: 시간 + 타입
                                                    <div class="flex items-center gap-2 mb-0.5">
                                                        <span class="text-[10px] font-mono text-grok-muted bg-grok-gray/50 px-1.5 py-0.5 rounded">
                                                            {time}
                                                        </span>
                                                        <span class=format!("text-[10px] font-medium uppercase tracking-wider {}", text_color)>
                                                            {type_label}
                                                        </span>
                                                    </div>
                                                    
                                                    // 라벨
                                                    <p class="text-sm text-white font-medium">
                                                        {marker.label.clone()}
                                                    </p>
                                                    
                                                    // 설명 (발화 내용 등)
                                                    {if has_desc {
                                                        view! {
                                                            <div class="mt-1 p-2 bg-grok-gray/30 rounded border-l-2 border-grok-border">
                                                                <p class="text-xs text-grok-muted/90 italic leading-relaxed">
                                                                    "\""{desc}"\""
                                                                </p>
                                                            </div>
                                                        }.into_any()
                                                    } else {
                                                        view! { <span></span> }.into_any()
                                                    }}
                                                </div>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            </div>
                        }.into_any()
                    }
                }}
            </div>
            
            // 하단 통계
            <div class="mt-4 pt-3 border-t border-grok-border">
                <div class="grid grid-cols-4 gap-2 text-center">
                    {move || {
                        let marks = markers.get();
                        let speaker_count = marks.iter().filter(|m| m.marker_type == MarkerType::SpeakerChange).count();
                        let emotion_count = marks.iter().filter(|m| m.marker_type == MarkerType::SentimentChange).count();
                        let insight_count = marks.iter().filter(|m| m.marker_type == MarkerType::Insight).count();
                        let solution_count = marks.iter().filter(|m| m.marker_type == MarkerType::SolutionProposed).count();
                        
                        view! {
                            <div class="p-2 bg-cyan-500/10 rounded">
                                <p class="text-lg font-bold text-cyan-400">{speaker_count}</p>
                                <p class="text-[10px] text-grok-muted uppercase">"발화"</p>
                            </div>
                            <div class="p-2 bg-orange-500/10 rounded">
                                <p class="text-lg font-bold text-orange-400">{emotion_count}</p>
                                <p class="text-[10px] text-grok-muted uppercase">"감정"</p>
                            </div>
                            <div class="p-2 bg-yellow-500/10 rounded">
                                <p class="text-lg font-bold text-yellow-400">{insight_count}</p>
                                <p class="text-[10px] text-grok-muted uppercase">"인사이트"</p>
                            </div>
                            <div class="p-2 bg-green-500/10 rounded">
                                <p class="text-lg font-bold text-green-400">{solution_count}</p>
                                <p class="text-[10px] text-grok-muted uppercase">"해결"</p>
                            </div>
                        }
                    }}
                </div>
            </div>
        </div>
    }
}
