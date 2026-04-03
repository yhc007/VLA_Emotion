use leptos::prelude::*;
use crate::api::client;
use crate::components::icons;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Report {
    pub summary: String,
    pub key_findings: Vec<String>,
    pub risk_level: String,
    pub recommendations: Vec<String>,
    pub counselor_notes: String,
}

#[derive(Clone, Debug)]
pub struct SolutionReport {
    pub problem_statement: String,
    pub root_causes: Vec<String>,
    pub priority: String,
    pub solutions: Vec<String>,
    pub action_items: Vec<String>,
    pub next_steps: String,
}

impl From<Report> for SolutionReport {
    fn from(r: Report) -> Self {
        SolutionReport {
            problem_statement: r.summary,
            root_causes: r.key_findings,
            priority: r.risk_level,
            solutions: r.recommendations,
            action_items: vec![],
            next_steps: r.counselor_notes,
        }
    }
}

#[component]
pub fn ReportViewer(
    case_id: ReadSignal<Option<String>>,
    on_close: impl Fn(()) + 'static + Clone,
) -> impl IntoView {
    let (report, set_report) = signal(Option::<SolutionReport>::None);
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(Option::<String>::None);
    
    // 리포트 로드
    Effect::new(move |_| {
        if let Some(cid) = case_id.get() {
            wasm_bindgen_futures::spawn_local(async move {
                set_loading.set(true);
                match client::generate_report(&cid).await {
                    Ok(r) => {
                        set_report.set(Some(r.into()));
                        set_error.set(None);
                    }
                    Err(e) => {
                        set_error.set(Some(e));
                    }
                }
                set_loading.set(false);
            });
        }
    });
    
    let priority_style = |level: &str| -> (&'static str, &'static str) {
        match level.to_lowercase().as_str() {
            "low" => ("text-green-400 border-green-400/30", "LOW"),
            "medium" => ("text-yellow-400 border-yellow-400/30", "MEDIUM"),
            "high" => ("text-red-400 border-red-400/30", "HIGH"),
            _ => ("text-grok-muted border-grok-border", "—"),
        }
    };
    
    let on_close_clone = on_close.clone();
    
    view! {
        // 모달 배경
        <div class="fixed inset-0 bg-black/95 z-50 flex items-center justify-center p-4">
            <div class="bg-grok-dark border border-grok-border rounded-md max-w-3xl w-full max-h-[90vh] overflow-hidden flex flex-col">
                // 헤더
                <div class="p-4 border-b border-grok-border flex items-center justify-between shrink-0">
                    <div class="flex items-center gap-3">
                        <div class="text-grok-blue">
                            {icons::lightbulb()}
                        </div>
                        <div>
                            <h2 class="text-sm font-medium text-white">"Solution Report"</h2>
                            <p class="text-xs text-grok-muted font-mono">
                                {move || case_id.get().map(|id| format!("CASE-{}", &id[..6.min(id.len())].to_uppercase())).unwrap_or_default()}
                            </p>
                        </div>
                    </div>
                    <button
                        on:click=move |_| on_close_clone(())
                        class="p-1 rounded hover:bg-grok-gray transition-colors text-grok-muted hover:text-white"
                    >
                        {icons::x()}
                    </button>
                </div>
                
                <div class="p-4 overflow-y-auto flex-1">
                    // 로딩 상태
                    <Show when=move || loading.get()>
                        <div class="text-center py-12">
                            <div class="inline-block w-8 h-8 border-2 border-grok-blue border-t-transparent rounded-full animate-spin" />
                            <p class="mt-3 text-sm text-grok-muted font-mono">"Generating solution..."</p>
                        </div>
                    </Show>
                    
                    // 에러 상태
                    <Show when=move || error.get().is_some()>
                        <div class="border border-red-500/30 rounded p-3 text-red-400 text-sm flex items-start gap-2">
                            {icons::alert_triangle()}
                            <p>{move || error.get().unwrap_or_default()}</p>
                        </div>
                    </Show>
                    
                    // 리포트 내용
                    {move || {
                        if let Some(r) = report.get() {
                            let (priority_class, priority_label) = priority_style(&r.priority);
                            let causes = r.root_causes.clone();
                            let solutions = r.solutions.clone();
                            
                            view! {
                                <div class="space-y-6">
                                    // 우선순위
                                    <div class="flex items-center justify-between">
                                        <span class="text-xs text-grok-muted font-mono uppercase">"Priority"</span>
                                        <div class="flex items-center gap-2">
                                            {icons::priority_indicator(&r.priority)}
                                            <span class=format!("px-2 py-0.5 text-xs font-mono border rounded {}", priority_class)>
                                                {priority_label}
                                            </span>
                                        </div>
                                    </div>
                                    
                                    // 문제 정의
                                    <div class="space-y-2">
                                        <h3 class="text-xs text-grok-muted font-mono uppercase tracking-wider flex items-center gap-2">
                                            {icons::crosshair()}
                                            "Problem Statement"
                                        </h3>
                                        <p class="text-sm text-grok-text leading-relaxed p-3 bg-grok-gray/50 rounded border border-grok-border">
                                            {r.problem_statement.clone()}
                                        </p>
                                    </div>
                                    
                                    // 근본 원인
                                    <div class="space-y-2">
                                        <h3 class="text-xs text-grok-muted font-mono uppercase tracking-wider flex items-center gap-2">
                                            {icons::search()}
                                            "Root Causes"
                                        </h3>
                                        <ul class="space-y-1">
                                            {causes.into_iter().map(|cause| {
                                                view! {
                                                    <li class="flex items-start gap-2 text-sm text-grok-text p-2 bg-grok-gray/30 rounded">
                                                        <span class="text-yellow-400 mt-0.5">{icons::arrow_right()}</span>
                                                        <span>{cause}</span>
                                                    </li>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </ul>
                                    </div>
                                    
                                    // 솔루션
                                    <div class="space-y-2">
                                        <h3 class="text-xs text-grok-muted font-mono uppercase tracking-wider flex items-center gap-2">
                                            {icons::zap()}
                                            "Proposed Solutions"
                                        </h3>
                                        <ul class="space-y-1">
                                            {solutions.into_iter().enumerate().map(|(i, sol)| {
                                                view! {
                                                    <li class="flex items-start gap-3 text-sm text-grok-text p-2 bg-grok-blue/10 rounded border border-grok-blue/20">
                                                        <span class="text-grok-blue font-mono text-xs mt-0.5">{format!("{:02}", i + 1)}</span>
                                                        <span>{sol}</span>
                                                    </li>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </ul>
                                    </div>
                                    
                                    // 다음 단계
                                    <div class="space-y-2">
                                        <h3 class="text-xs text-grok-muted font-mono uppercase tracking-wider flex items-center gap-2">
                                            {icons::arrow_right()}
                                            "Next Steps"
                                        </h3>
                                        <p class="text-sm text-grok-text leading-relaxed p-3 bg-grok-gray/50 rounded border border-grok-border whitespace-pre-wrap">
                                            {r.next_steps.clone()}
                                        </p>
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! { <div /> }.into_any()
                        }
                    }}
                </div>
                
                // 푸터
                <div class="p-4 border-t border-grok-border shrink-0">
                    <div class="flex items-center justify-between text-xs text-grok-muted font-mono">
                        <span>"Generated by Claude AI"</span>
                        <span>{move || {
                            let now = js_sys::Date::new_0();
                            format!("{}-{:02}-{:02}", now.get_full_year(), now.get_month() + 1, now.get_date())
                        }}</span>
                    </div>
                </div>
            </div>
        </div>
    }
}
