use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use crate::components::transcript::ReasoningResult;

// ─── JS 바인딩: vis.js 인터랙티브 그래프 ───

#[wasm_bindgen(inline_js = r#"

// ── 글로벌 그래프 상태 ──
let _vla_network = null;
let _vla_nodes = null;
let _vla_edges = null;
let _vla_known_nodes = new Set();
let _vla_known_edges = new Set();
let _vla_pulse_timers = [];
let _vla_stats = { nodes: 0, edges: 0, inferred: 0 };

// 색상 매핑
const NODE_COLORS = {
    Problem: { bg: '#ef4444', border: '#dc2626', highlight: '#f87171' },
    OperationalProblem: { bg: '#ef4444', border: '#dc2626', highlight: '#f87171' },
    StrategicProblem: { bg: '#b91c1c', border: '#991b1b', highlight: '#dc2626' },
    FinancialProblem: { bg: '#ef4444', border: '#dc2626', highlight: '#f87171' },
    HRProblem: { bg: '#dc2626', border: '#b91c1c', highlight: '#ef4444' },
    TechnicalProblem: { bg: '#f87171', border: '#ef4444', highlight: '#fca5a5' },
    Cause: { bg: '#f97316', border: '#ea580c', highlight: '#fb923c' },
    InternalCause: { bg: '#f97316', border: '#ea580c', highlight: '#fb923c' },
    ExternalCause: { bg: '#ea580c', border: '#c2410c', highlight: '#f97316' },
    ProcessCause: { bg: '#fbbf24', border: '#f59e0b', highlight: '#fcd34d' },
    ResourceCause: { bg: '#f97316', border: '#ea580c', highlight: '#fb923c' },
    Impact: { bg: '#a855f7', border: '#9333ea', highlight: '#c084fc' },
    RevenueImpact: { bg: '#9333ea', border: '#7e22ce', highlight: '#a855f7' },
    CostImpact: { bg: '#a855f7', border: '#9333ea', highlight: '#c084fc' },
    TimeImpact: { bg: '#a78bfa', border: '#8b5cf6', highlight: '#c4b5fd' },
    QualityImpact: { bg: '#7e22ce', border: '#6b21a8', highlight: '#9333ea' },
    ReputationImpact: { bg: '#6b21a8', border: '#581c87', highlight: '#7e22ce' },
    Solution: { bg: '#22c55e', border: '#16a34a', highlight: '#4ade80' },
    ProcessImprovement: { bg: '#16a34a', border: '#15803d', highlight: '#22c55e' },
    ResourceAllocation: { bg: '#22c55e', border: '#16a34a', highlight: '#4ade80' },
    TechSolution: { bg: '#14b8a6', border: '#0d9488', highlight: '#2dd4bf' },
    TrainingSolution: { bg: '#0d9488', border: '#0f766e', highlight: '#14b8a6' },
    Stakeholder: { bg: '#3b82f6', border: '#2563eb', highlight: '#60a5fa' },
    Customer: { bg: '#2563eb', border: '#1d4ed8', highlight: '#3b82f6' },
    Employee: { bg: '#3b82f6', border: '#2563eb', highlight: '#60a5fa' },
    Management: { bg: '#1d4ed8', border: '#1e40af', highlight: '#2563eb' },
    Resource: { bg: '#eab308', border: '#ca8a04', highlight: '#facc15' },
};

const EDGE_COLORS = {
    causes: '#f97316',
    impacts: '#a855f7',
    affects: '#3b82f6',
    solves: '#22c55e',
    requires: '#eab308',
    mitigates: '#14b8a6',
    rootCause: '#dc2626',
    highPriority: '#ef4444',
    permanentSolution: '#16a34a',
};

const EDGE_LABELS = {
    causes: '원인', impacts: '영향', affects: '영향받음',
    solves: '해결', requires: '필요', mitigates: '완화',
    rootCause: '근본원인', highPriority: '높은우선순위', permanentSolution: '영구해결',
};

const NODE_SHAPES = {
    Problem: 'diamond', Cause: 'triangle', Impact: 'star',
    Solution: 'square', Stakeholder: 'dot', Resource: 'hexagon',
};

function getParentType(t) {
    if (t.includes('Problem')) return 'Problem';
    if (t.includes('Cause')) return 'Cause';
    if (t.includes('Impact')) return 'Impact';
    if (t.includes('Solution') || t.includes('Improvement') || t.includes('Allocation')) return 'Solution';
    if (t.includes('Stakeholder') || t === 'Customer' || t === 'Employee' || t === 'Management' || t === 'Partner') return 'Stakeholder';
    if (t.includes('Resource')) return 'Resource';
    return 'Other';
}

// ── 그래프 초기화 ──
export function init_graph(container_id) {
    const container = document.getElementById(container_id);
    if (!container) return;

    _vla_nodes = new vis.DataSet([]);
    _vla_edges = new vis.DataSet([]);
    _vla_known_nodes = new Set();
    _vla_known_edges = new Set();
    _vla_stats = { nodes: 0, edges: 0, inferred: 0 };

    const options = {
        physics: {
            enabled: true,
            solver: 'forceAtlas2Based',
            forceAtlas2Based: {
                gravitationalConstant: -60,
                centralGravity: 0.008,
                springLength: 140,
                springConstant: 0.05,
                damping: 0.5,
                avoidOverlap: 0.7,
            },
            stabilization: { iterations: 80, fit: true },
        },
        interaction: {
            hover: true,
            tooltipDelay: 150,
            zoomView: true,
            dragView: true,
        },
        edges: {
            smooth: { type: 'curvedCW', roundness: 0.15 },
        },
        layout: { improvedLayout: true },
    };

    _vla_network = new vis.Network(container, { nodes: _vla_nodes, edges: _vla_edges }, options);

    // 노드 클릭 → detail-panel 업데이트
    _vla_network.on('click', (params) => {
        const panel = document.getElementById('graph-detail-panel');
        if (!panel) return;
        if (params.nodes.length > 0) {
            const nodeId = params.nodes[0];
            const node = _vla_nodes.get(nodeId);
            const connEdges = _vla_network.getConnectedEdges(nodeId);
            const connNodes = _vla_network.getConnectedNodes(nodeId);
            let html = `<div class="text-sm font-medium text-white mb-1">${node.label}</div>`;
            html += `<div class="text-xs text-gray-400 mb-2">${node.title || ''}</div>`;
            html += `<div class="text-xs text-gray-500">Connections: ${connNodes.length}</div>`;
            connEdges.forEach(eid => {
                const edge = _vla_edges.get(eid);
                if (edge) {
                    const dir = edge.from === nodeId ? '→' : '←';
                    const otherId = edge.from === nodeId ? edge.to : edge.from;
                    const other = _vla_nodes.get(otherId);
                    html += `<div class="text-xs mt-1"><span class="text-gray-500">${dir}</span> <span style="color:${edge.color?.color || '#888'}">${edge.label || ''}</span> <span class="text-gray-300">${other?.label || ''}</span></div>`;
                }
            });
            panel.innerHTML = html;
        } else {
            panel.innerHTML = '<div class="text-xs text-gray-500">Click a node</div>';
        }
    });
}

// ── 노드/엣지 증분 추가 ──
export function add_entities(entities_json) {
    if (!_vla_nodes) return;
    const entities = JSON.parse(entities_json);

    entities.forEach(e => {
        const eType = e.type || e.entity_type || 'Unknown';
        const nodeId = eType + '_' + e.text.replace(/\s+/g, '_');
        if (_vla_known_nodes.has(nodeId)) return;
        _vla_known_nodes.add(nodeId);

        const parentType = getParentType(eType);
        const colors = NODE_COLORS[eType] || NODE_COLORS[parentType] || { bg: '#6b7280', border: '#4b5563', highlight: '#9ca3af' };
        const shape = NODE_SHAPES[parentType] || 'dot';

        _vla_nodes.add({
            id: nodeId,
            label: e.text,
            color: { background: colors.bg, border: colors.border, highlight: { background: colors.highlight, border: colors.bg } },
            shape: shape,
            size: 22,
            font: { color: '#fff', size: 12, face: 'Pretendard, sans-serif' },
            borderWidth: 2,
            shadow: { enabled: true, color: colors.bg + '40', size: 8, x: 0, y: 0 },
            title: `${e.text}\n${parentType} / ${eType}`,
            // 진입 애니메이션용 커스텀 데이터
            _parentType: parentType,
            _subType: eType,
        });

        _vla_stats.nodes++;
        pulse_node(nodeId, colors.highlight);
    });
}

export function add_relations(relations_json) {
    if (!_vla_edges || !_vla_nodes) return;
    const relations = JSON.parse(relations_json);

    relations.forEach(r => {
        // source/target 노드 ID 찾기
        const fromId = find_node_id(r.subject);
        const toId = find_node_id(r.object);
        if (!fromId || !toId) return;

        const edgeKey = `${fromId}__${r.name}__${toId}`;
        if (_vla_known_edges.has(edgeKey)) return;
        _vla_known_edges.add(edgeKey);

        const color = EDGE_COLORS[r.name] || '#6b7280';
        const label = EDGE_LABELS[r.name] || r.name;
        const isInferred = r.is_inferred || false;

        _vla_edges.add({
            from: fromId,
            to: toId,
            label: label,
            color: { color: color, highlight: color, opacity: 0.85 },
            width: isInferred ? 1.5 : 2.5,
            arrows: { to: { enabled: true, scaleFactor: 1.0 } },
            font: { size: 10, color: '#aaa', face: 'Pretendard, sans-serif', strokeWidth: 2, strokeColor: '#000' },
            smooth: { type: 'curvedCW', roundness: 0.15 },
            dashes: isInferred ? [6, 3] : false,
            title: `${r.name} (${isInferred ? '추론' : '추출'})`,
        });

        _vla_stats.edges++;
        if (isInferred) _vla_stats.inferred++;
    });
}

// ── 유틸 ──
function find_node_id(label) {
    const allNodes = _vla_nodes.get();
    // 정확 일치
    let found = allNodes.find(n => n.label === label);
    if (found) return found.id;
    // 부분 일치
    found = allNodes.find(n => label.includes(n.label) || n.label.includes(label));
    return found ? found.id : null;
}

function pulse_node(nodeId, color) {
    // 새 노드 진입 시 size 펄스 애니메이션
    const originalSize = 22;
    const pulseSize = 32;
    _vla_nodes.update({ id: nodeId, size: pulseSize });
    const timer = setTimeout(() => {
        if (_vla_nodes.get(nodeId)) {
            _vla_nodes.update({ id: nodeId, size: originalSize });
        }
    }, 600);
    _vla_pulse_timers.push(timer);
}

export function get_stats() {
    return JSON.stringify(_vla_stats);
}

export function fit_graph() {
    if (_vla_network) _vla_network.fit({ animation: { duration: 400, easingFunction: 'easeInOutQuad' } });
}

export function toggle_physics() {
    if (!_vla_network) return false;
    const opts = _vla_network.physics?.options?.enabled !== false;
    _vla_network.setOptions({ physics: { enabled: !opts } });
    return !opts;
}

export function clear_graph() {
    if (_vla_nodes) _vla_nodes.clear();
    if (_vla_edges) _vla_edges.clear();
    _vla_known_nodes.clear();
    _vla_known_edges.clear();
    _vla_pulse_timers.forEach(t => clearTimeout(t));
    _vla_pulse_timers = [];
    _vla_stats = { nodes: 0, edges: 0, inferred: 0 };
}

export function destroy_graph() {
    clear_graph();
    if (_vla_network) { _vla_network.destroy(); _vla_network = null; }
}

"#)]
extern "C" {
    fn init_graph(container_id: &str);
    fn add_entities(entities_json: &str);
    fn add_relations(relations_json: &str);
    fn get_stats() -> String;
    fn fit_graph();
    fn toggle_physics() -> bool;
    fn clear_graph();
    fn destroy_graph();
}

// ─── 컴포넌트 ───

#[component]
pub fn ReasoningGraph(
    #[prop(into)] result: Signal<Option<ReasoningResult>>,
) -> impl IntoView {
    // 그래프 초기화 여부
    let (initialized, set_initialized) = signal(false);
    let (node_count, set_node_count) = signal(0_usize);
    let (edge_count, set_edge_count) = signal(0_usize);
    let (inferred_count, set_inferred_count) = signal(0_usize);
    let (latest_summary, set_latest_summary) = signal(String::new());
    let (critical, set_critical) = signal(false);
    let (priority, set_priority) = signal("normal".to_string());

    // 마운트 후 vis.js 초기화
    Effect::new(move |_| {
        if !initialized.get() {
            // DOM이 렌더링될 때까지 약간 대기
            let handle = gloo_timers::callback::Timeout::new(100, move || {
                init_graph("vla-reasoning-graph");
                set_initialized.set(true);
                log::info!("vis.js reasoning graph initialized");
            });
            handle.forget();
        }
    });

    // reasoning_result 변경 시 그래프에 증분 추가
    Effect::new(move |_| {
        if !initialized.get() { return; }

        if let Some(r) = result.get() {
            // 1. 엔티티 → 노드 추가
            let entities_json = serde_json::to_string(&r.entities).unwrap_or_default();
            add_entities(&entities_json);

            // 2. 관계 → 엣지 추가
            // RelationInfo를 JS가 기대하는 형태로 변환
            let relations: Vec<serde_json::Value> = r.relations.iter().map(|rel| {
                serde_json::json!({
                    "name": rel.name,
                    "subject": rel.subject,
                    "object": rel.object,
                    "is_inferred": false,
                })
            }).collect();
            let relations_json = serde_json::to_string(&relations).unwrap_or_default();
            add_relations(&relations_json);

            // 3. 통계 갱신
            if let Ok(stats_str) = serde_json::from_str::<serde_json::Value>(&get_stats()) {
                if let Some(n) = stats_str.get("nodes").and_then(|v| v.as_u64()) {
                    set_node_count.set(n as usize);
                }
                if let Some(e) = stats_str.get("edges").and_then(|v| v.as_u64()) {
                    set_edge_count.set(e as usize);
                }
                if let Some(i) = stats_str.get("inferred").and_then(|v| v.as_u64()) {
                    set_inferred_count.set(i as usize);
                }
            }

            // 4. 메타 정보
            set_latest_summary.set(r.summary.clone());
            set_critical.set(r.critical);
            set_priority.set(r.priority.clone());

            // 5. fit 뷰
            fit_graph();
        }
    });

    // 그래프 컨트롤 핸들러
    let on_fit = move |_| { fit_graph(); };
    let on_toggle_physics = move |_| { toggle_physics(); };
    let on_clear = move |_| {
        clear_graph();
        set_node_count.set(0);
        set_edge_count.set(0);
        set_inferred_count.set(0);
        set_latest_summary.set(String::new());
    };

    view! {
        <div class="grok-card rounded-md p-4 flex flex-col" style="min-height: 420px;">
            // ── 헤더 ──
            <div class="flex items-center justify-between mb-3">
                <h2 class="text-xs font-medium text-grok-muted uppercase tracking-wider flex items-center gap-2">
                    <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                            d="M13 10V3L4 14h7v7l9-11h-7z" />
                    </svg>
                    "SPO Knowledge Graph"
                    // 라이브 인디케이터
                    <Show when=move || node_count.get() != 0>
                        <span class="relative flex h-2 w-2 ml-1">
                            <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-green-400 opacity-75"></span>
                            <span class="relative inline-flex rounded-full h-2 w-2 bg-green-500"></span>
                        </span>
                    </Show>
                </h2>

                // 상태 배지 + 통계
                <div class="flex items-center gap-2">
                    {move || {
                        if critical.get() {
                            view! { <span class="px-2 py-0.5 bg-red-500/30 text-red-300 text-[10px] rounded-full animate-pulse">"CRITICAL"</span> }.into_any()
                        } else if priority.get() == "high" {
                            view! { <span class="px-2 py-0.5 bg-yellow-500/30 text-yellow-300 text-[10px] rounded-full">"HIGH"</span> }.into_any()
                        } else if node_count.get() > 0 {
                            view! { <span class="px-2 py-0.5 bg-green-500/20 text-green-300 text-[10px] rounded-full">"LIVE"</span> }.into_any()
                        } else {
                            view! { <span class="px-2 py-0.5 bg-gray-500/20 text-gray-400 text-[10px] rounded-full">"IDLE"</span> }.into_any()
                        }
                    }}
                    <span class="text-[10px] text-grok-muted font-mono">
                        {move || format!("N:{} E:{}", node_count.get(), edge_count.get())}
                    </span>
                </div>
            </div>

            // ── 그래프 영역 ──
            <div class="relative flex-1 rounded-md overflow-hidden border border-grok-border" style="min-height: 280px;">
                // vis.js 컨테이너
                <div id="vla-reasoning-graph" class="w-full h-full" style="min-height: 280px; background: #08081a;"></div>

                // 대기 상태 오버레이
                <Show when=move || node_count.get() == 0>
                    <div class="absolute inset-0 flex flex-col items-center justify-center bg-black/40 pointer-events-none">
                        <div class="text-2xl mb-2 opacity-60">"🧠"</div>
                        <div class="text-xs text-grok-muted">"Waiting for analysis..."</div>
                        <div class="text-[10px] text-grok-muted mt-1">"Speak to build the knowledge graph"</div>
                    </div>
                </Show>

                // 컨트롤 버튼
                <Show when=move || node_count.get() != 0>
                    <div class="absolute bottom-2 right-2 flex gap-1.5 z-10">
                        <button
                            on:click=on_fit
                            class="px-2 py-1 rounded text-[10px] bg-black/70 text-gray-300 hover:bg-grok-blue hover:text-white border border-gray-700 transition-colors"
                            title="Fit to view"
                        >"Fit"</button>
                        <button
                            on:click=on_toggle_physics
                            class="px-2 py-1 rounded text-[10px] bg-black/70 text-gray-300 hover:bg-grok-blue hover:text-white border border-gray-700 transition-colors"
                            title="Toggle physics"
                        >"Physics"</button>
                        <button
                            on:click=on_clear
                            class="px-2 py-1 rounded text-[10px] bg-black/70 text-red-400 hover:bg-red-600 hover:text-white border border-gray-700 transition-colors"
                            title="Clear graph"
                        >"Clear"</button>
                    </div>
                </Show>

                // 범례
                <Show when=move || node_count.get() != 0>
                    <div class="absolute top-2 left-2 flex flex-col gap-0.5 z-10">
                        <div class="flex items-center gap-1.5 text-[9px] text-gray-400">
                            <div class="w-2 h-2 rounded-sm" style="background: #ef4444;"></div>
                            "Problem"
                        </div>
                        <div class="flex items-center gap-1.5 text-[9px] text-gray-400">
                            <div class="w-2 h-2 rounded-sm" style="background: #f97316;"></div>
                            "Cause"
                        </div>
                        <div class="flex items-center gap-1.5 text-[9px] text-gray-400">
                            <div class="w-2 h-2 rounded-sm" style="background: #a855f7;"></div>
                            "Impact"
                        </div>
                        <div class="flex items-center gap-1.5 text-[9px] text-gray-400">
                            <div class="w-2 h-2 rounded-sm" style="background: #22c55e;"></div>
                            "Solution"
                        </div>
                        <div class="flex items-center gap-1.5 text-[9px] text-gray-400">
                            <div class="w-2 h-2 rounded-sm" style="background: #3b82f6;"></div>
                            "Stakeholder"
                        </div>
                    </div>
                </Show>
            </div>

            // ── 디테일 패널 ──
            <div class="mt-2 flex gap-2">
                // 선택된 노드 정보
                <div id="graph-detail-panel" class="flex-1 p-2 rounded bg-black/30 border border-grok-border text-xs min-h-[32px]">
                    <div class="text-xs text-gray-500">"Click a node"</div>
                </div>

                // 최신 분석 요약
                <Show when=move || !latest_summary.get().is_empty()>
                    <div class="flex-1 p-2 rounded bg-grok-blue/5 border border-grok-blue/20 text-xs">
                        <div class="text-grok-blue text-[10px] font-medium mb-0.5">"Latest Insight"</div>
                        <div class="text-gray-300 line-clamp-2">
                            {move || latest_summary.get()}
                        </div>
                    </div>
                </Show>
            </div>
        </div>
    }
}
