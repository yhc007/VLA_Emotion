use crate::types::*;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::{BTreeMap, HashMap};
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// 인메모리 의미 그래프 엔진 (petgraph 기반)
///
/// SPO 트리플과 감정 데이터를 노드/엣지로 변환하여
/// 실시간 의미 그래프를 구축한다.
#[derive(Clone)]
pub struct SemanticGraph {
    graph: DiGraph<GraphNode, GraphEdge>,
    /// 엔티티 ID → NodeIndex 매핑
    entity_index: HashMap<String, NodeIndex>,
    /// 타임스탬프 → NodeIndex 리스트 (시간순 조회)
    time_index: BTreeMap<DateTime<Utc>, Vec<NodeIndex>>,
    /// 마지막 발화 노드 ID (TEMPORAL_NEXT 엣지용)
    last_utterance_idx: Option<NodeIndex>,
    /// 그래프 버전 (GraphUpdated 이벤트용)
    version: u64,
    /// 세션 ID
    session_id: Uuid,
}

impl SemanticGraph {
    pub fn new(session_id: Uuid) -> Self {
        info!(session_id = %session_id, "의미 그래프 생성");
        Self {
            graph: DiGraph::new(),
            entity_index: HashMap::new(),
            time_index: BTreeMap::new(),
            last_utterance_idx: None,
            version: 0,
            session_id,
        }
    }

    /// SPO 트리플을 그래프에 추가
    pub fn add_spo_triple(&mut self, triple: &SPOTriple) -> GraphUpdated {
        let mut added_nodes = Vec::new();
        let mut added_edges = Vec::new();

        // 1. 발화 노드 생성
        let utterance_node = GraphNode::utterance(
            &triple.source_utterance,
            triple.timestamp,
            triple.session_id,
        );
        let utt_idx = self.graph.add_node(utterance_node.clone());
        self.time_index
            .entry(triple.timestamp)
            .or_default()
            .push(utt_idx);
        added_nodes.push(utterance_node.clone());

        // 2. TEMPORAL_NEXT 엣지 (직전 발화 → 현재 발화)
        if let Some(prev_idx) = self.last_utterance_idx {
            let prev_id = &self.graph[prev_idx].id;
            let edge = GraphEdge::temporal_next(prev_id, &self.graph[utt_idx].id);
            self.graph.add_edge(prev_idx, utt_idx, edge.clone());
            added_edges.push(edge);
        }
        self.last_utterance_idx = Some(utt_idx);

        // 3. Subject 노드 (없으면 생성)
        let subj_is_new = !self.entity_index.contains_key(&triple.subject.id);
        let subj_idx = self.get_or_create_entity_node(&triple.subject);
        if subj_is_new {
            added_nodes.push(self.graph[subj_idx].clone());
        }

        // 4. Object 노드 (없으면 생성)
        let obj_is_new = !self.entity_index.contains_key(&triple.object.id);
        let obj_idx = self.get_or_create_entity_node(&triple.object);
        if obj_is_new {
            added_nodes.push(self.graph[obj_idx].clone());
        }

        // 5. SPO_RELATION 엣지 (Subject → Object, relation = predicate.verb)
        let spo_edge = GraphEdge::spo_relation(
            &triple.subject.id,
            &triple.object.id,
            &triple.predicate.verb,
            &triple.source_utterance,
        );
        self.graph.add_edge(subj_idx, obj_idx, spo_edge.clone());
        added_edges.push(spo_edge);

        self.version += 1;
        debug!(
            version = self.version,
            nodes = self.graph.node_count(),
            edges = self.graph.edge_count(),
            "그래프 업데이트"
        );

        GraphUpdated {
            added_nodes,
            added_edges,
            version: self.version,
            timestamp: triple.timestamp,
            session_id: self.session_id,
        }
    }

    /// 감정 결과를 그래프에 어노테이션
    pub fn annotate_emotion(&mut self, emotion: &EmotionResult) -> Option<GraphUpdated> {
        let mut added_nodes = Vec::new();
        let mut added_edges = Vec::new();

        // 감정 상태 노드 생성
        let emo_node = GraphNode::emotion_state(emotion);
        let emo_idx = self.graph.add_node(emo_node.clone());
        added_nodes.push(emo_node);

        // 가장 최근 발화 노드에 HAS_EMOTION 엣지 연결
        if let Some(utt_idx) = self.last_utterance_idx {
            let utt_id = self.graph[utt_idx].id.clone();
            let emo_id = format!("emo_{}", emotion.id);

            let edge = GraphEdge::has_emotion(&utt_id, &emo_id);
            self.graph.add_edge(utt_idx, emo_idx, edge.clone());
            added_edges.push(edge);

            // 발화 노드에 감정 어노테이션 추가
            let utt_node = &mut self.graph[utt_idx];
            utt_node.emotion_annotations.push(EmotionAnnotation {
                emotion_id: emotion.id,
                valence: emotion.valence,
                arousal: emotion.arousal,
                timestamp: emotion.timestamp,
            });

            self.version += 1;

            Some(GraphUpdated {
                added_nodes,
                added_edges,
                version: self.version,
                timestamp: emotion.timestamp,
                session_id: self.session_id,
            })
        } else {
            warn!("감정 어노테이션 실패: 연결할 발화 노드 없음");
            None
        }
    }

    /// 말-표정 불일치 감지 후 CONTRADICTS 엣지 추가
    pub fn add_contradiction(
        &mut self,
        spo_triple: &SPOTriple,
        emotion: &EmotionResult,
    ) -> Option<GraphEdge> {
        if emotion.contradicts_polarity(&spo_triple.predicate.polarity) {
            // 가장 최근 발화 노드와 감정 노드를 연결
            let utt_idx = self.last_utterance_idx;
            let emo_id = format!("emo_{}", emotion.id);
            let emo_idx = self.find_node_by_id(&emo_id);

            let edge = if let (Some(src), Some(tgt)) = (utt_idx, emo_idx) {
                let utt_id = self.graph[src].id.clone();
                let edge = GraphEdge::contradicts(&utt_id, &emo_id);
                self.graph.add_edge(src, tgt, edge.clone());
                edge
            } else {
                // 인덱스를 찾지 못하면 엣지만 반환 (그래프에 추가하지 않음)
                let source_id = utt_idx
                    .map(|idx| self.graph[idx].id.clone())
                    .unwrap_or_else(|| format!("utt_{}", spo_triple.id));
                GraphEdge::contradicts(&source_id, &emo_id)
            };

            info!(
                spo_polarity = ?spo_triple.predicate.polarity,
                emotion_valence = emotion.valence,
                "말-표정 불일치 감지!"
            );

            Some(edge)
        } else {
            None
        }
    }

    // ─── 조회 메서드 ───

    /// 노드 수
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// 엣지 수
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// 현재 버전
    pub fn version(&self) -> u64 {
        self.version
    }

    /// 특정 엔티티의 관련 SPO 관계 조회
    pub fn get_entity_relations(&self, entity_id: &str) -> Vec<(&GraphEdge, &GraphNode)> {
        let mut relations = Vec::new();

        if let Some(&idx) = self.entity_index.get(entity_id) {
            for edge_idx in self.graph.edges(idx) {
                let target = &self.graph[edge_idx.target()];
                relations.push((edge_idx.weight(), target));
            }
        }

        relations
    }

    /// 시간 범위 내 발화 노드 조회
    pub fn get_utterances_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<&GraphNode> {
        self.time_index
            .range(from..=to)
            .flat_map(|(_, indices)| indices.iter().map(|&idx| &self.graph[idx]))
            .filter(|node| node.node_type == NodeType::Utterance)
            .collect()
    }

    /// 최근 N개 감정 어노테이션 조회
    pub fn get_recent_emotions(&self, limit: usize) -> Vec<&EmotionAnnotation> {
        let mut emotions: Vec<&EmotionAnnotation> = self.graph
            .node_indices()
            .flat_map(|idx| self.graph[idx].emotion_annotations.iter())
            .collect();

        emotions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        emotions.truncate(limit);
        emotions
    }

    /// 그래프 요약 (디버깅용)
    pub fn summary(&self) -> String {
        let node_types: HashMap<&NodeType, usize> = self.graph
            .node_indices()
            .fold(HashMap::new(), |mut acc, idx| {
                *acc.entry(&self.graph[idx].node_type).or_insert(0) += 1;
                acc
            });

        format!(
            "SemanticGraph v{}: {} nodes, {} edges | {:?}",
            self.version,
            self.graph.node_count(),
            self.graph.edge_count(),
            node_types,
        )
    }

    /// 시각화용 그래프 데이터 내보내기
    /// 반환: (노드 목록, 엣지 목록)
    /// 노드: (id, label, type)
    /// 엣지: (source, target, label, weight)
    pub fn export_for_visualization(&self) -> (Vec<(String, String, String)>, Vec<(String, String, String, f32)>) {
        let nodes: Vec<(String, String, String)> = self.graph
            .node_indices()
            .map(|idx| {
                let node = &self.graph[idx];
                (
                    node.id.clone(),
                    node.label.clone(),
                    format!("{:?}", node.node_type),
                )
            })
            .collect();

        let edges: Vec<(String, String, String, f32)> = self.graph
            .edge_indices()
            .filter_map(|idx| {
                let edge = self.graph.edge_weight(idx)?;
                let (src_idx, tgt_idx) = self.graph.edge_endpoints(idx)?;
                Some((
                    self.graph[src_idx].id.clone(),
                    self.graph[tgt_idx].id.clone(),
                    edge.relation.clone(),
                    edge.weight,
                ))
            })
            .collect();

        (nodes, edges)
    }

    // ─── 내부 헬퍼 ───

    fn get_or_create_entity_node(&mut self, entity: &Entity) -> NodeIndex {
        if let Some(&idx) = self.entity_index.get(&entity.id) {
            return idx;
        }

        let node = GraphNode::from_entity(
            &entity.id,
            &entity.name,
            &entity.entity_type,
            self.session_id,
        );
        let idx = self.graph.add_node(node);
        self.entity_index.insert(entity.id.clone(), idx);
        idx
    }

    fn find_node_by_id(&self, id: &str) -> Option<NodeIndex> {
        // 엔티티 인덱스에 없으면 전체 검색
        self.entity_index.get(id).copied().or_else(|| {
            self.graph.node_indices()
                .find(|&idx| self.graph[idx].id == id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::spo::*;

    #[test]
    fn test_add_spo_triple() {
        let session_id = Uuid::new_v4();
        let mut graph = SemanticGraph::new(session_id);

        let triple = SPOTriple::new(
            Entity::person("나"),
            Predicate::new("힘들다", Polarity::Negative).with_intensity(0.9),
            Entity::organization("회사"),
            "나는 회사가 힘들어",
            "speaker_01",
            session_id,
            0.85,
        );

        let update = graph.add_spo_triple(&triple);
        assert!(update.added_nodes.len() >= 2);
        assert!(!update.added_edges.is_empty());
        assert_eq!(graph.version(), 1);
    }

    #[test]
    fn test_emotion_annotation() {
        let session_id = Uuid::new_v4();
        let mut graph = SemanticGraph::new(session_id);

        // 먼저 SPO 추가 (발화 노드 생성)
        let triple = SPOTriple::new(
            Entity::person("나"),
            Predicate::new("힘들다", Polarity::Negative),
            Entity::organization("회사"),
            "나는 회사가 힘들어",
            "speaker_01",
            session_id,
            0.85,
        );
        graph.add_spo_triple(&triple);

        // 감정 어노테이션
        let emotion = EmotionResult::new(
            EmotionType::Sad,
            -0.7,
            0.3,
            "speaker_01",
            session_id,
        );
        let result = graph.annotate_emotion(&emotion);
        assert!(result.is_some());
    }

    #[test]
    fn test_contradiction_detection() {
        let session_id = Uuid::new_v4();
        let mut graph = SemanticGraph::new(session_id);

        // 긍정적 발화
        let triple = SPOTriple::new(
            Entity::person("나"),
            Predicate::new("좋다", Polarity::Positive),
            Entity::concept("상황"),
            "나는 상황이 좋아",
            "speaker_01",
            session_id,
            0.8,
        );
        graph.add_spo_triple(&triple);

        // 부정적 표정 (불일치!)
        let emotion = EmotionResult::new(
            EmotionType::Sad,
            -0.7,
            0.3,
            "speaker_01",
            session_id,
        );

        let contradiction = graph.add_contradiction(&triple, &emotion);
        assert!(contradiction.is_some(), "말-표정 불일치가 감지되어야 함");
    }
}
