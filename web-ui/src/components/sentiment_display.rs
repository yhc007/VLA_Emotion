use leptos::prelude::*;
use crate::app::SentimentData;
use crate::components::icons;

#[component]
pub fn SentimentDisplay(
    sentiments: ReadSignal<Vec<SentimentData>>,
) -> impl IntoView {
    let latest_sentiment = move || {
        sentiments.get().last().cloned()
    };
    
    let sentiment_label = |sentiment: &str| -> (&'static str, &'static str) {
        match sentiment.to_lowercase().as_str() {
            "positive" | "happy" | "satisfied" => ("Positive", "text-green-400"),
            "negative" | "frustrated" | "angry" => ("Frustrated", "text-red-400"),
            "confused" | "uncertain" => ("Confused", "text-yellow-400"),
            "neutral" => ("Neutral", "text-grok-muted"),
            _ => ("Analyzing", "text-grok-muted"),
        }
    };
    
    view! {
        <div class="grok-card rounded-md p-4">
            <h2 class="text-xs font-medium text-grok-muted uppercase tracking-wider flex items-center gap-2 mb-4">
                {icons::gauge()}
                "Client Sentiment"
            </h2>
            
            {move || {
                if let Some(sentiment) = latest_sentiment() {
                    let (label, color_class) = sentiment_label(&sentiment.sentiment);
                    let confidence = (sentiment.confidence * 100.0) as i32;
                    
                    view! {
                        <div class="space-y-4">
                            <div class="flex items-center justify-between">
                                <div class="flex items-center gap-3">
                                    <div class="w-10 h-10 rounded border border-grok-border bg-grok-gray flex items-center justify-center">
                                        <div class=color_class>
                                            {icons::sentiment_icon(&sentiment.sentiment, 5)}
                                        </div>
                                    </div>
                                    <div>
                                        <p class=format!("text-lg font-semibold {}", color_class)>{label}</p>
                                        <p class="text-xs text-grok-muted font-mono">{format!("{}% confidence", confidence)}</p>
                                    </div>
                                </div>
                                {icons::priority_indicator(&sentiment.sentiment)}
                            </div>
                            
                            // 프로그레스 바
                            <div class="h-1 bg-grok-gray rounded-full overflow-hidden">
                                <div 
                                    class="h-full bg-grok-blue transition-all duration-300"
                                    style=format!("width: {}%", confidence)
                                />
                            </div>
                            
                            // 인사이트 힌트
                            <div class="flex items-start gap-2 p-2 bg-grok-gray/50 rounded border border-grok-border">
                                <div class="text-grok-blue mt-0.5">
                                    {icons::lightbulb()}
                                </div>
                                <p class="text-xs text-grok-text">
                                    {match sentiment.sentiment.to_lowercase().as_str() {
                                        "confused" | "uncertain" => "Client needs clarification. Consider breaking down the problem.",
                                        "frustrated" | "negative" | "angry" => "Pain point detected. Focus on understanding the root cause.",
                                        "positive" | "happy" | "satisfied" => "Client is engaged. Good moment to propose solutions.",
                                        _ => "Gathering more data for analysis..."
                                    }}
                                </p>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="flex items-center gap-3 py-2">
                            <div class="w-10 h-10 rounded border border-grok-border bg-grok-gray flex items-center justify-center text-grok-muted">
                                {icons::sentiment_icon("neutral", 5)}
                            </div>
                            <div>
                                <p class="text-sm text-grok-muted">"Waiting for input..."</p>
                                <p class="text-xs text-grok-muted font-mono">"—"</p>
                            </div>
                        </div>
                    }.into_any()
                }
            }}
            
            // 히스토리
            <div class="mt-4 pt-4 border-t border-grok-border">
                <p class="text-xs text-grok-muted mb-2 font-mono uppercase tracking-wider">"History"</p>
                <div class="flex gap-1">
                    {move || {
                        let list = sentiments.get();
                        if list.is_empty() {
                            view! {
                                <p class="text-xs text-grok-muted">"No data"</p>
                            }.into_any()
                        } else {
                            view! {
                                {list.iter().rev().take(8).map(|s| {
                                    let (_, color) = sentiment_label(&s.sentiment);
                                    view! {
                                        <div 
                                            class=format!("w-6 h-6 rounded border border-grok-border bg-grok-gray flex items-center justify-center {} hover:border-grok-blue transition-colors", color)
                                            title=s.sentiment.clone()
                                        >
                                            {icons::sentiment_icon(&s.sentiment, 3)}
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            }.into_any()
                        }
                    }}
                </div>
            </div>
        </div>
    }
}
