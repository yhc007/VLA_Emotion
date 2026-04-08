use leptos::prelude::*;
use crate::components::transcript::{ReasoningResult, EntityInfo, RelationInfo};

/// Neurosymbolic 추론 그래프 시각화
#[component]
pub fn ReasoningGraph(
    #[prop(into)] result: Signal<Option<ReasoningResult>>,
) -> impl IntoView {
    // 그래프 데이터 계산
    let graph_data = move || {
        result.get().map(|r| {
            // 엔티티를 노드로 변환
            let nodes: Vec<GraphNode> = r.entities.iter().enumerate().map(|(i, e)| {
                let (color, category) = entity_style(&e.entity_type);
                GraphNode {
                    id: i,
                    label: e.text.clone(),
                    entity_type: e.entity_type.clone(),
                    color: color.to_string(),
                    category: category.to_string(),
                    x: calculate_x(i, r.entities.len()),
                    y: calculate_y(&e.entity_type),
                }
            }).collect();
            
            // 관계를 엣지로 변환
            let edges: Vec<GraphEdge> = r.relations.iter().filter_map(|rel| {
                let from = r.entities.iter().position(|e| e.text == rel.subject)?;
                let to = r.entities.iter().position(|e| e.text == rel.object)?;
                Some(GraphEdge {
                    from,
                    to,
                    label: rel.name.clone(),
                    color: relation_color(&rel.name),
                })
            }).collect();
            
            (nodes, edges, r.solutions.clone(), r.critical, r.priority.clone())
        })
    };
    
    view! {
        <div class="grok-card rounded-md p-4">
            <h2 class="text-xs font-medium text-grok-muted uppercase tracking-wider flex items-center gap-2 mb-3">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" 
                        d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
                </svg>
                "Neurosymbolic Reasoning"
            </h2>
            
            <Show 
                when=move || graph_data().is_some()
                fallback=|| view! {
                    <div class="text-center text-grok-muted py-8">
                        <div class="text-2xl mb-2">"🧠"</div>
                        <div class="text-sm">"분석 결과를 기다리는 중..."</div>
                    </div>
                }
            >
                {move || {
                    let (nodes, edges, solutions, critical, priority) = graph_data().unwrap();
                    let solutions_for_show = solutions.clone();
                    let solutions_for_render = solutions.clone();
                    
                    view! {
                        <div class="space-y-4">
                            // 상태 배지
                            <div class="flex items-center gap-2">
                                {if critical {
                                    view! { <span class="px-2 py-1 bg-red-500/30 text-red-300 text-xs rounded animate-pulse">"⚠️ CRITICAL"</span> }.into_any()
                                } else if priority == "high" {
                                    view! { <span class="px-2 py-1 bg-yellow-500/30 text-yellow-300 text-xs rounded">"🔶 HIGH PRIORITY"</span> }.into_any()
                                } else {
                                    view! { <span class="px-2 py-1 bg-green-500/30 text-green-300 text-xs rounded">"✅ NORMAL"</span> }.into_any()
                                }}
                                <span class="text-grok-muted text-xs">
                                    {format!("{} entities, {} relations", nodes.len(), edges.len())}
                                </span>
                            </div>
                            
                            // SVG 그래프
                            <svg 
                                viewBox="0 0 400 250" 
                                class="w-full h-48 bg-grok-darker rounded border border-grok-border"
                            >
                                // 범례
                                <g transform="translate(10, 10)">
                                    <rect x="0" y="0" width="8" height="8" fill="#ef4444" rx="2"/>
                                    <text x="12" y="8" fill="#9ca3af" font-size="8">"Problem"</text>
                                    <rect x="60" y="0" width="8" height="8" fill="#f97316" rx="2"/>
                                    <text x="72" y="8" fill="#9ca3af" font-size="8">"Cause"</text>
                                    <rect x="110" y="0" width="8" height="8" fill="#22c55e" rx="2"/>
                                    <text x="122" y="8" fill="#9ca3af" font-size="8">"Solution"</text>
                                    <rect x="170" y="0" width="8" height="8" fill="#eab308" rx="2"/>
                                    <text x="182" y="8" fill="#9ca3af" font-size="8">"Impact"</text>
                                </g>
                                
                                // 엣지 (화살표)
                                <defs>
                                    <marker id="arrowhead" markerWidth="10" markerHeight="7" 
                                        refX="9" refY="3.5" orient="auto">
                                        <polygon points="0 0, 10 3.5, 0 7" fill="#6b7280"/>
                                    </marker>
                                </defs>
                                
                                {edges.iter().map(|edge| {
                                    let from_node = &nodes[edge.from];
                                    let to_node = &nodes[edge.to];
                                    let mid_x = (from_node.x + to_node.x) / 2.0;
                                    let mid_y = (from_node.y + to_node.y) / 2.0 - 10.0;
                                    
                                    view! {
                                        <g class="animate-fade-in">
                                            <line 
                                                x1={from_node.x} y1={from_node.y}
                                                x2={to_node.x} y2={to_node.y}
                                                stroke={edge.color.clone()}
                                                stroke-width="2"
                                                marker-end="url(#arrowhead)"
                                                opacity="0.7"
                                            />
                                            <text 
                                                x={mid_x} y={mid_y}
                                                fill="#9ca3af"
                                                font-size="9"
                                                text-anchor="middle"
                                            >
                                                {edge.label.clone()}
                                            </text>
                                        </g>
                                    }
                                }).collect::<Vec<_>>()}
                                
                                // 노드
                                {nodes.iter().map(|node| {
                                    view! {
                                        <g class="animate-scale-in" style="transform-origin: center">
                                            <circle 
                                                cx={node.x} cy={node.y} r="20"
                                                fill={node.color.clone()}
                                                opacity="0.8"
                                                class="transition-all duration-300 hover:opacity-100"
                                            />
                                            <text 
                                                x={node.x} y={node.y + 4.0}
                                                fill="white"
                                                font-size="8"
                                                text-anchor="middle"
                                                font-weight="500"
                                            >
                                                {truncate_text(&node.label, 8)}
                                            </text>
                                        </g>
                                    }
                                }).collect::<Vec<_>>()}
                            </svg>
                            
                            // 엔티티 목록
                            <div class="grid grid-cols-2 gap-2">
                                {nodes.iter().map(|node| {
                                    view! {
                                        <div 
                                            class="flex items-center gap-2 px-2 py-1 rounded text-xs"
                                            style=format!("background: {}20; border-left: 3px solid {}", node.color, node.color)
                                        >
                                            <span class="font-medium text-white">{node.label.clone()}</span>
                                            <span class="text-grok-muted">{node.category.clone()}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                            
                            // 해결책
                            <Show when=move || !solutions_for_show.is_empty()>
                                <div class="border-t border-grok-border pt-3 mt-3">
                                    <div class="text-xs text-grok-muted mb-2">"💡 Solutions"</div>
                                    {solutions_for_render.iter().map(|sol| {
                                        let status_class = if sol.blocked {
                                            "bg-red-500/20 border-red-500/30"
                                        } else if sol.permanent {
                                            "bg-green-500/20 border-green-500/30"
                                        } else {
                                            "bg-blue-500/20 border-blue-500/30"
                                        };
                                        let status_icon = if sol.blocked { "🚫" } else if sol.permanent { "⭐" } else { "✓" };
                                        
                                        view! {
                                            <div class=format!("px-3 py-2 rounded border {} text-sm", status_class)>
                                                <span class="mr-2">{status_icon}</span>
                                                <span class="text-white font-medium">{sol.name.clone()}</span>
                                                <span class="text-grok-muted ml-2">{format!("→ {}", sol.problem)}</span>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            </Show>
                        </div>
                    }
                }}
            </Show>
        </div>
    }
}

#[derive(Clone)]
struct GraphNode {
    id: usize,
    label: String,
    entity_type: String,
    color: String,
    category: String,
    x: f64,
    y: f64,
}

#[derive(Clone)]
struct GraphEdge {
    from: usize,
    to: usize,
    label: String,
    color: String,
}

fn entity_style(entity_type: &str) -> (&str, &str) {
    match entity_type {
        t if t.contains("Problem") => ("#ef4444", "Problem"),
        t if t.contains("Cause") => ("#f97316", "Cause"),
        t if t.contains("Solution") => ("#22c55e", "Solution"),
        t if t.contains("Impact") => ("#eab308", "Impact"),
        t if t.contains("Stakeholder") => ("#8b5cf6", "Stakeholder"),
        t if t.contains("Resource") => ("#06b6d4", "Resource"),
        _ => ("#6b7280", "Other"),
    }
}

fn relation_color(relation: &str) -> String {
    match relation {
        "causes" => "#f97316".to_string(),
        "solves" => "#22c55e".to_string(),
        "impacts" => "#eab308".to_string(),
        "affects" => "#8b5cf6".to_string(),
        "requires" => "#06b6d4".to_string(),
        _ => "#6b7280".to_string(),
    }
}

fn calculate_x(index: usize, total: usize) -> f64 {
    let padding = 60.0;
    let width = 400.0 - padding * 2.0;
    if total <= 1 {
        200.0
    } else {
        padding + (index as f64 / (total - 1) as f64) * width
    }
}

fn calculate_y(entity_type: &str) -> f64 {
    match entity_type {
        t if t.contains("Cause") => 60.0,
        t if t.contains("Problem") => 125.0,
        t if t.contains("Impact") => 125.0,
        t if t.contains("Solution") => 190.0,
        t if t.contains("Stakeholder") => 190.0,
        _ => 125.0,
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() > max_len {
        format!("{}…", text.chars().take(max_len).collect::<String>())
    } else {
        text.to_string()
    }
}
