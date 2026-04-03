use crate::types::*;
use chrono::{Duration, Utc};
use tracing::{debug, warn};

/// 심리 상태 분석기
///
/// 의미 그래프의 SPO 패턴과 감정 데이터를 종합하여
/// 실시간 심리 상태를 추론하고 이상 패턴을 탐지한다.
#[derive(Clone)]
pub struct PsychAnalyzer {
    /// 최근 감정 기록 (슬라이딩 윈도우)
    emotion_history: Vec<EmotionResult>,
    /// 최근 SPO 트리플 기록
    spo_history: Vec<SPOTriple>,
    /// 말-표정 불일치 횟수
    mismatch_count: u32,
    /// 분석 윈도우 크기 (초)
    window_secs: i64,
    /// 최대 히스토리 크기
    max_history: usize,
}

impl PsychAnalyzer {
    pub fn new(window_secs: i64, max_history: usize) -> Self {
        Self {
            emotion_history: Vec::new(),
            spo_history: Vec::new(),
            mismatch_count: 0,
            window_secs,
            max_history,
        }
    }

    /// 새 감정 데이터 수신
    pub fn record_emotion(&mut self, emotion: EmotionResult) {
        self.emotion_history.push(emotion);
        self.trim_history();
    }

    /// 새 SPO 트리플 수신
    pub fn record_spo(&mut self, triple: SPOTriple) {
        self.spo_history.push(triple);
        self.trim_history();
    }

    /// 말-표정 불일치 기록
    pub fn record_mismatch(&mut self) {
        self.mismatch_count += 1;
    }

    /// 발화(SPO) 기록 수
    pub fn utterance_count(&self) -> usize {
        self.spo_history.len()
    }

    /// 종합 심리 상태 분석
    pub fn analyze(&self, session_id: uuid::Uuid) -> PsychState {
        let mut state = PsychState::new(session_id);
        state.analysis_window_secs = self.window_secs as u64;

        // 1. 감정 추이 분석
        state.emotion_trend = self.analyze_emotion_trend();

        // 2. 전반적 심리 상태 판별
        state.overall_state = self.determine_overall_state();

        // 3. 말-표정 일치도 (coherence)
        state.coherence_score = self.calculate_coherence();

        // 4. 이상 패턴 탐지
        state.anomalies = self.detect_anomalies();

        if state.is_concerning() {
            warn!(
                state = ?state.overall_state,
                coherence = state.coherence_score,
                anomalies = state.anomalies.len(),
                "우려되는 심리 상태 감지"
            );
        } else {
            debug!(state = ?state.overall_state, "심리 상태 정상");
        }

        state
    }

    // ─── 내부 분석 로직 ───

    /// 감정 추이 분석
    ///
    /// 시간 윈도우 기반 필터링을 우선 사용하되, 모든 데이터가
    /// 동일 시간대에 들어온 경우(배치/데모) 최근 N개 시퀀스로 폴백한다.
    fn analyze_emotion_trend(&self) -> EmotionTrend {
        let recent = self.get_recent_emotions();
        if recent.len() < 2 {
            return EmotionTrend::Stable;
        }

        // valence 변화율 계산 (입력 순서 = 시간 순서)
        let valences: Vec<f32> = recent.iter().map(|e| e.valence).collect();
        let first_half_avg = avg(&valences[..valences.len() / 2]);
        let second_half_avg = avg(&valences[valences.len() / 2..]);
        let delta = second_half_avg - first_half_avg;

        // 변동성 계산
        let volatility = self.calculate_volatility(&valences);

        if volatility > 0.5 {
            EmotionTrend::Volatile
        } else if delta > 0.2 {
            EmotionTrend::Improving
        } else if delta < -0.2 {
            EmotionTrend::Declining
        } else {
            EmotionTrend::Stable
        }
    }

    /// 최근 감정 데이터 가져오기
    /// 시간 윈도우 내 데이터가 충분하면 사용, 아니면 최근 N개로 폴백
    fn get_recent_emotions(&self) -> Vec<&EmotionResult> {
        let window_start = Utc::now() - Duration::seconds(self.window_secs);
        let by_time: Vec<&EmotionResult> = self.emotion_history.iter()
            .filter(|e| e.timestamp > window_start)
            .collect();

        // 시간 윈도우 내 2개 이상이면 사용
        if by_time.len() >= 2 {
            return by_time;
        }

        // 폴백: 최근 max_history개를 시퀀스(입력 순서) 기반으로 사용
        let take = self.max_history.min(self.emotion_history.len());
        self.emotion_history[self.emotion_history.len() - take..].iter().collect()
    }

    /// 전반적 심리 상태 결정
    fn determine_overall_state(&self) -> PsychStateType {
        let recent_emotions = self.get_recent_emotions();

        if recent_emotions.is_empty() {
            return PsychStateType::Normal;
        }

        let avg_valence = avg(&recent_emotions.iter().map(|e| e.valence).collect::<Vec<_>>());
        let avg_arousal = avg(&recent_emotions.iter().map(|e| e.arousal).collect::<Vec<_>>());

        // 불일치가 지속되면 회피적
        if self.mismatch_count >= 3 {
            return PsychStateType::Evasive;
        }

        // valence-arousal 공간에서 상태 결정
        match (avg_valence, avg_arousal) {
            (v, a) if v < -0.5 && a > 0.6 => PsychStateType::Agitated,
            (v, _) if v < -0.5 => PsychStateType::Distressed,
            (v, a) if v < -0.2 && a > 0.4 => PsychStateType::Stressed,
            (v, _) if v < -0.2 => PsychStateType::Withdrawn,
            (v, a) if v > 0.3 && a > 0.6 => PsychStateType::Elevated,
            _ => PsychStateType::Normal,
        }
    }

    /// 말-표정 일치도 계산 (0.0 ~ 1.0)
    fn calculate_coherence(&self) -> f32 {
        let total = self.emotion_history.len() as f32;
        if total == 0.0 {
            return 1.0;
        }
        let mismatch_ratio = self.mismatch_count as f32 / total;
        (1.0 - mismatch_ratio).max(0.0)
    }

    /// 이상 패턴 탐지
    fn detect_anomalies(&self) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // 1. 감정 급변 탐지
        if let Some(anomaly) = self.detect_sudden_emotion_change() {
            anomalies.push(anomaly);
        }

        // 2. 말-표정 불일치 지속
        if self.mismatch_count >= 3 {
            anomalies.push(Anomaly {
                anomaly_type: AnomalyType::SpeechEmotionMismatch,
                severity: if self.mismatch_count >= 5 {
                    Severity::High
                } else {
                    Severity::Medium
                },
                evidence: vec![
                    format!("말-표정 불일치 {}회 감지", self.mismatch_count),
                ],
                timestamp: Utc::now(),
            });
        }

        // 3. 주제 반복 탐지
        if let Some(anomaly) = self.detect_topic_repetition() {
            anomalies.push(anomaly);
        }

        // 4. 부정적 감정 에스컬레이션
        if let Some(anomaly) = self.detect_negative_escalation() {
            anomalies.push(anomaly);
        }

        anomalies
    }

    /// 감정 급변 탐지
    fn detect_sudden_emotion_change(&self) -> Option<Anomaly> {
        if self.emotion_history.len() < 2 {
            return None;
        }

        let last_two: Vec<&EmotionResult> = self.emotion_history.iter().rev().take(2).collect();
        let delta = (last_two[0].valence - last_two[1].valence).abs();

        if delta > 0.8 {
            Some(Anomaly {
                anomaly_type: AnomalyType::EmotionSuddenChange,
                severity: Severity::High,
                evidence: vec![
                    format!("valence 급변: {:.2} → {:.2} (Δ={:.2})",
                        last_two[1].valence, last_two[0].valence, delta),
                ],
                timestamp: Utc::now(),
            })
        } else if delta > 0.5 {
            Some(Anomaly {
                anomaly_type: AnomalyType::EmotionSuddenChange,
                severity: Severity::Medium,
                evidence: vec![
                    format!("valence 변화: {:.2} → {:.2} (Δ={:.2})",
                        last_two[1].valence, last_two[0].valence, delta),
                ],
                timestamp: Utc::now(),
            })
        } else {
            None
        }
    }

    /// 주제 반복 탐지 (동일 목적어가 3회 이상 등장)
    fn detect_topic_repetition(&self) -> Option<Anomaly> {
        let mut object_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

        for triple in &self.spo_history {
            // _implicit 플레이스홀더는 실제 주제가 아니므로 제외
            if triple.object.name == "_implicit" {
                continue;
            }
            *object_counts.entry(&triple.object.name).or_insert(0) += 1;
        }

        let repeated: Vec<_> = object_counts.iter()
            .filter(|(_, &count)| count >= 3)
            .map(|(&name, &count)| format!("'{}' {}회 언급", name, count))
            .collect();

        if !repeated.is_empty() {
            Some(Anomaly {
                anomaly_type: AnomalyType::TopicRepetition,
                severity: Severity::Low,
                evidence: repeated,
                timestamp: Utc::now(),
            })
        } else {
            None
        }
    }

    /// 부정적 감정 에스컬레이션 탐지
    fn detect_negative_escalation(&self) -> Option<Anomaly> {
        if self.emotion_history.len() < 3 {
            return None;
        }

        let recent: Vec<f32> = self.emotion_history.iter()
            .rev()
            .take(5)
            .map(|e| e.valence)
            .collect();

        // 연속 하강 패턴 체크
        let consecutive_decline = recent.windows(2)
            .filter(|w| w[0] < w[1]) // 역순이므로 최신이 더 낮으면
            .count();

        if consecutive_decline >= 3 && recent[0] < -0.3 {
            Some(Anomaly {
                anomaly_type: AnomalyType::NegativeEscalation,
                severity: Severity::High,
                evidence: vec![
                    format!("연속 {}회 감정 하락, 현재 valence: {:.2}", consecutive_decline, recent[0]),
                ],
                timestamp: Utc::now(),
            })
        } else {
            None
        }
    }

    /// 변동성 계산 (표준편차)
    fn calculate_volatility(&self, values: &[f32]) -> f32 {
        if values.len() < 2 {
            return 0.0;
        }
        let mean = avg(values);
        let variance = values.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f32>() / values.len() as f32;
        variance.sqrt()
    }

    /// 히스토리 크기 제한
    fn trim_history(&mut self) {
        if self.emotion_history.len() > self.max_history {
            self.emotion_history.drain(..self.emotion_history.len() - self.max_history);
        }
        if self.spo_history.len() > self.max_history {
            self.spo_history.drain(..self.spo_history.len() - self.max_history);
        }
    }
}

fn avg(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f32>() / values.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use uuid::Uuid;

    #[test]
    fn test_normal_state() {
        let analyzer = PsychAnalyzer::new(60, 100);
        let state = analyzer.analyze(Uuid::new_v4());
        assert_eq!(state.overall_state, PsychStateType::Normal);
        assert_eq!(state.coherence_score, 1.0);
    }

    #[test]
    fn test_mismatch_detection() {
        let mut analyzer = PsychAnalyzer::new(60, 100);
        let session_id = Uuid::new_v4();

        // 감정 기록
        for _ in 0..5 {
            analyzer.record_emotion(EmotionResult::new(
                EmotionType::Happy, 0.7, 0.5, "speaker_01", session_id,
            ));
            analyzer.record_mismatch();
        }

        let state = analyzer.analyze(session_id);
        assert_eq!(state.overall_state, PsychStateType::Evasive);
        assert!(state.coherence_score < 0.5);
        assert!(!state.anomalies.is_empty());
    }

    #[test]
    fn test_emotion_trend_declining() {
        let mut analyzer = PsychAnalyzer::new(60, 100);
        let session_id = Uuid::new_v4();

        // 점진적으로 부정적으로 변하는 감정 시퀀스
        let valences = [0.2, 0.0, -0.2, -0.4, -0.6];
        for v in &valences {
            analyzer.record_emotion(EmotionResult::new(
                EmotionType::Sad, *v, 0.4, "speaker_01", session_id,
            ));
        }

        let state = analyzer.analyze(session_id);
        assert_eq!(state.emotion_trend, EmotionTrend::Declining,
            "점진적 부정화 → Declining 추이");
    }

    #[test]
    fn test_emotion_trend_volatile() {
        let mut analyzer = PsychAnalyzer::new(60, 100);
        let session_id = Uuid::new_v4();

        // 급격한 감정 변동
        let valences = [0.8, -0.7, 0.6, -0.8, 0.5];
        for v in &valences {
            let emo = if *v > 0.0 { EmotionType::Happy } else { EmotionType::Sad };
            analyzer.record_emotion(EmotionResult::new(
                emo, *v, 0.5, "speaker_01", session_id,
            ));
        }

        let state = analyzer.analyze(session_id);
        assert_eq!(state.emotion_trend, EmotionTrend::Volatile,
            "급격한 감정 변동 → Volatile 추이");
    }
}
