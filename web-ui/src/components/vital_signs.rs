//! 생체 신호 표시 컴포넌트 (심박수, 긴장도 등)

use leptos::prelude::*;
use crate::components::rppg::{HeartRateResult, TensionLevel};

/// 생체 신호 표시 컴포넌트
#[component]
pub fn VitalSigns(
    heart_rate: ReadSignal<HeartRateResult>,
) -> impl IntoView {
    let tension = move || TensionLevel::from_result(&heart_rate.get());
    
    view! {
        <div class="grok-card rounded-md p-4">
            <div class="flex items-center justify-between mb-3">
                <h2 class="text-xs font-medium text-grok-muted uppercase tracking-wider flex items-center gap-2">
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                            d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z" />
                    </svg>
                    "Vital Signs"
                </h2>
                // 긴장도 표시
                <div class="flex items-center gap-2">
                    <span class="text-lg">{move || tension().emoji()}</span>
                    <span class="text-xs font-mono text-grok-muted">{move || tension().label()}</span>
                </div>
            </div>
            
            <div class="space-y-3">
                // 심박수
                <div class="flex items-center justify-between">
                    <div class="flex items-center gap-2">
                        <div class="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
                        <span class="text-sm text-grok-muted">"Heart Rate"</span>
                    </div>
                    <div class="flex items-baseline gap-1">
                        <span class="text-2xl font-bold text-white font-mono">
                            {move || format!("{:.0}", heart_rate.get().bpm)}
                        </span>
                        <span class="text-xs text-grok-muted">"BPM"</span>
                    </div>
                </div>
                
                // 신뢰도 바
                <div class="h-1 bg-grok-border rounded-full overflow-hidden">
                    <div 
                        class="h-full bg-gradient-to-r from-red-500 to-green-500 transition-all duration-500"
                        style=move || format!("width: {}%", heart_rate.get().confidence * 100.0)
                    />
                </div>
                <div class="flex justify-between text-xs text-grok-muted">
                    <span>"Confidence"</span>
                    <span>{move || format!("{:.0}%", heart_rate.get().confidence * 100.0)}</span>
                </div>
                
                // HRV
                <div class="flex items-center justify-between pt-2 border-t border-grok-border">
                    <span class="text-sm text-grok-muted">"HRV (SDNN)"</span>
                    <div class="flex items-baseline gap-1">
                        <span class="text-lg font-semibold text-white font-mono">
                            {move || format!("{:.0}", heart_rate.get().hrv_sdnn)}
                        </span>
                        <span class="text-xs text-grok-muted">"ms"</span>
                    </div>
                </div>
                
                // 스트레스 지수
                <div class="flex items-center justify-between">
                    <span class="text-sm text-grok-muted">"Stress Index"</span>
                    <div class="flex items-center gap-2">
                        <div class="w-20 h-2 bg-grok-border rounded-full overflow-hidden">
                            <div 
                                class="h-full transition-all duration-500"
                                style=move || {
                                    let stress = heart_rate.get().stress_index;
                                    let color = if stress < 0.3 { 
                                        "bg-green-500" 
                                    } else if stress < 0.6 { 
                                        "bg-yellow-500" 
                                    } else { 
                                        "bg-red-500" 
                                    };
                                    format!("width: {}%; background-color: {}", 
                                        stress * 100.0,
                                        match color {
                                            "bg-green-500" => "#22c55e",
                                            "bg-yellow-500" => "#eab308",
                                            _ => "#ef4444"
                                        }
                                    )
                                }
                            />
                        </div>
                        <span class="text-sm font-mono text-grok-muted w-10 text-right">
                            {move || format!("{:.0}%", heart_rate.get().stress_index * 100.0)}
                        </span>
                    </div>
                </div>
                
                // 신호 품질 (디버그용)
                <div class="flex items-center justify-between text-xs">
                    <span class="text-grok-muted">"Signal Quality"</span>
                    <span class=move || {
                        let quality = heart_rate.get().signal_quality;
                        if quality > 0.7 { "text-green-400" }
                        else if quality > 0.4 { "text-yellow-400" }
                        else { "text-red-400" }
                    }>
                        {move || {
                            let quality = heart_rate.get().signal_quality;
                            if quality > 0.7 { "Good" }
                            else if quality > 0.4 { "Fair" }
                            else { "Poor" }
                        }}
                    </span>
                </div>
            </div>
        </div>
    }
}

/// 미니 심박수 표시 (비디오 오버레이용)
#[component]
pub fn HeartRateMini(
    bpm: ReadSignal<f64>,
    confidence: ReadSignal<f64>,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2 bg-black/60 rounded px-2 py-1">
            <div class="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
            <span class="text-white font-mono text-sm font-bold">
                {move || format!("{:.0}", bpm.get())}
            </span>
            <span class="text-grok-muted text-xs">"BPM"</span>
            <Show when=move || confidence.get() < 0.5>
                <span class="text-yellow-400 text-xs">"⚠"</span>
            </Show>
        </div>
    }
}
