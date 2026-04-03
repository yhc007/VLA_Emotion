use crate::types::spo::*;
use super::rules::*;
use std::collections::HashMap;
use tracing::{debug, info};
use uuid::Uuid;

/// 한국어 문장에서 SPO 트리플을 추출하는 규칙 기반 파서.
///
/// Phase 1에서는 조사 기반 규칙으로 동작하며,
/// 향후 LLM 기반 추출기로 교체/보강 가능하다.
pub struct SPOExtractor {
    /// 직전 발화의 주어 (주어 생략 시 승계용)
    last_subject: Option<Entity>,
    /// 엔티티 정규화 캐시 (같은 이름 → 같은 Entity)
    entity_cache: HashMap<String, Entity>,
}

impl SPOExtractor {
    pub fn new() -> Self {
        Self {
            last_subject: None,
            entity_cache: HashMap::new(),
        }
    }

    /// 텍스트에서 SPO 트리플을 추출
    pub fn extract(
        &mut self,
        text: &str,
        speaker_id: &str,
        session_id: Uuid,
    ) -> Vec<SPOTriple> {
        let sentences = self.split_sentences(text);
        let mut triples = Vec::new();

        for sentence in &sentences {
            // 문장 내 절 분리 (복문 처리)
            let clauses = self.split_clauses(sentence);

            for clause in &clauses {
                if let Some(triple) = self.extract_from_clause(clause, speaker_id, session_id) {
                    self.last_subject = Some(triple.subject.clone());
                    triples.push(triple);
                }
            }
        }

        if !triples.is_empty() {
            info!(
                count = triples.len(),
                text = text,
                "SPO 트리플 추출 완료"
            );
        }

        triples
    }

    /// 단일 절(clause)에서 SPO 추출
    fn extract_from_clause(
        &mut self,
        clause: &str,
        speaker_id: &str,
        session_id: Uuid,
    ) -> Option<SPOTriple> {
        let clause = clause.trim();
        if clause.is_empty() {
            return None;
        }

        let tokens = self.tokenize(clause);
        if tokens.is_empty() {
            return None;
        }

        debug!(clause = clause, tokens = ?tokens, "절 토큰화");

        let subject = self.find_subject(&tokens, speaker_id);
        let object = self.find_object(&tokens);
        let predicate = self.find_predicate(&tokens, clause);

        // 최소한 주어와 술어가 있어야 트리플 생성
        let subject = subject?;
        let predicate = predicate?;
        let object = object.unwrap_or_else(|| Entity::concept("_implicit"));

        let confidence = self.calculate_confidence(&subject, &predicate, &object);

        Some(SPOTriple::new(
            subject,
            predicate,
            object,
            clause,
            speaker_id,
            session_id,
            confidence,
        ))
    }

    /// 주어 추출: 조사 "은/는/이/가" 앞의 명사
    fn find_subject(&mut self, tokens: &[Token], speaker_id: &str) -> Option<Entity> {
        for token in tokens {
            // 긴 조사부터 매칭 (에서 > 이, 께서 > 가)
            let mut particles: Vec<&&str> = SUBJECT_PARTICLES.iter().collect();
            particles.sort_by(|a, b| b.len().cmp(&a.len()));

            for particle in particles {
                if token.text.ends_with(*particle) {
                    let noun = token.text.trim_end_matches(*particle);
                    if !noun.is_empty() {
                        let noun = self.strip_suffix(noun);
                        return Some(self.resolve_entity(&noun));
                    }
                }
            }
        }

        // 주어 생략 시 직전 주어 승계
        if let Some(ref last) = self.last_subject {
            debug!("주어 생략 → 직전 주어 승계: {}", last.name);
            return Some(last.clone());
        }

        // 그래도 없으면 화자 자신
        Some(self.resolve_entity(speaker_id))
    }

    /// 목적어 추출: 조사 "을/를/에/에게" 앞의 명사
    fn find_object(&mut self, tokens: &[Token]) -> Option<Entity> {
        for token in tokens {
            // 긴 조사부터 매칭
            let mut particles: Vec<&&str> = OBJECT_PARTICLES.iter().collect();
            particles.sort_by(|a, b| b.len().cmp(&a.len()));

            for particle in particles {
                if token.text.ends_with(*particle) {
                    let noun = token.text.trim_end_matches(*particle);
                    if !noun.is_empty() {
                        let noun = self.strip_suffix(noun);
                        return Some(self.resolve_entity(&noun));
                    }
                }
            }
        }
        None
    }

    /// 술어 추출: 부정어 결합 + 복합 술어 감지 + 극성/강도 판별
    fn find_predicate(&self, tokens: &[Token], clause: &str) -> Option<Predicate> {
        if tokens.is_empty() {
            return None;
        }

        // 1. 복합 술어 감지: "그만둘까 생각중이야" → "그만두다"가 핵심
        if let Some(core_verb) = self.detect_compound_predicate(tokens) {
            let polarity = self.detect_polarity_for_verb(&core_verb, clause);
            let intensity = self.detect_intensity(clause);
            return Some(Predicate::new(&core_verb, polarity).with_intensity(intensity));
        }

        // 2. 마지막 토큰이 술어(용언)라고 가정 (SOV 어순)
        let verb_token = tokens.last()?;
        let verb = self.normalize_verb(&verb_token.text);

        if verb.is_empty() {
            return None;
        }

        // 3. 부정어 결합: "안" + "좋아" → "안좋다" (Negative)
        let has_negation = self.has_negation_adverb(tokens);
        let base_polarity = self.detect_polarity_for_verb(&verb, clause);

        let polarity = if has_negation {
            // 부정어가 있으면 극성 반전
            match base_polarity {
                Polarity::Positive => Polarity::Negative,
                Polarity::Negative => Polarity::Negative, // 이중부정은 부정 유지
                Polarity::Neutral => Polarity::Negative,  // "안" + 중립 → 부정
            }
        } else {
            base_polarity
        };

        let intensity = self.detect_intensity(clause);
        let verb_display = if has_negation {
            format!("안{}", verb)
        } else {
            verb
        };

        Some(Predicate::new(&verb_display, polarity).with_intensity(intensity))
    }

    /// 복합 술어 감지: "X할까/하려고 생각중이야" → X를 핵심 술어로 추출
    fn detect_compound_predicate(&self, tokens: &[Token]) -> Option<String> {
        if tokens.len() < 2 {
            return None;
        }

        // 마지막 토큰이 보조 용언인지 확인
        let last = self.normalize_verb(&tokens.last()?.text);
        let is_auxiliary = AUXILIARY_VERB_MARKERS.iter()
            .any(|aux| last.contains(aux));

        if !is_auxiliary {
            return None;
        }

        // 앞 토큰에서 핵심 술어 추출
        for token in tokens.iter().rev().skip(1) {
            // 먼저 어미 제거로 정규화
            let normalized = self.normalize_verb(&token.text);

            // 받침 복원: "그만둘" → "그만두" (ㄹ 받침 제거)
            let restored = self.restore_stem(&normalized);

            if !restored.is_empty() && restored != last {
                debug!(
                    original = token.text.as_str(),
                    normalized = normalized.as_str(),
                    restored = restored.as_str(),
                    "복합 술어 → 핵심 술어 추출"
                );
                return Some(restored);
            }
        }

        None
    }

    /// 어간 복원: 활용으로 인한 받침을 제거하여 원형에 가깝게 복원
    /// "그만둘" → "그만두", "갈" → "가"
    fn restore_stem(&self, verb: &str) -> String {
        if verb.is_empty() {
            return verb.to_string();
        }
        let chars: Vec<char> = verb.chars().collect();
        let last_ch = *chars.last().unwrap();

        // 마지막 글자에 받침이 있으면 제거 시도
        if let Some(without_jong) = self.remove_final_consonant(last_ch) {
            let mut result: String = chars[..chars.len() - 1].iter().collect();
            result.push(without_jong);
            return result;
        }

        verb.to_string()
    }

    /// 부정 부사 존재 여부 확인 ("안", "못")
    fn has_negation_adverb(&self, tokens: &[Token]) -> bool {
        // 마지막 토큰(술어) 직전의 부정 부사만 인정
        if tokens.len() < 2 {
            return false;
        }
        // 술어 앞 2개 토큰 내에서 부정어 찾기
        let search_range = if tokens.len() >= 3 {
            &tokens[tokens.len() - 3..tokens.len() - 1]
        } else {
            &tokens[..tokens.len() - 1]
        };

        search_range.iter().any(|t| {
            NEGATION_ADVERBS.iter().any(|neg| t.text == *neg)
        })
    }

    /// 술어 단어 자체의 극성 판별 (전체 문장이 아닌 술어 중심)
    fn detect_polarity_for_verb(&self, verb: &str, clause: &str) -> Polarity {
        // 1차: 술어 자체의 극성 확인
        let verb_neg = NEGATIVE_MARKERS.iter().any(|m| verb.contains(m));
        let verb_pos = POSITIVE_MARKERS.iter().any(|m| verb.contains(m));

        if verb_neg && !verb_pos {
            return Polarity::Negative;
        }
        if verb_pos && !verb_neg {
            return Polarity::Positive;
        }

        // 2차: 절 전체에서 극성 판별 (폴백)
        let neg_count = NEGATIVE_MARKERS.iter()
            .filter(|m| clause.contains(**m))
            .count();
        let pos_count = POSITIVE_MARKERS.iter()
            .filter(|m| clause.contains(**m))
            .count();

        if neg_count > pos_count {
            Polarity::Negative
        } else if pos_count > neg_count {
            Polarity::Positive
        } else {
            Polarity::Neutral
        }
    }

    /// 강도 판별: 강도 부사 검사
    fn detect_intensity(&self, text: &str) -> f32 {
        for (intensifier, value) in INTENSIFIERS {
            if text.contains(intensifier) {
                return *value;
            }
        }
        0.5 // 기본 강도
    }

    /// 접미사 제거: "들" (복수), "님" (존칭) 등
    fn strip_suffix(&self, noun: &str) -> String {
        let suffixes = &["들"];
        let mut result = noun.to_string();
        for suffix in suffixes {
            if result.ends_with(suffix) {
                let stripped = result.trim_end_matches(suffix);
                // 접미사 제거 후 최소 1글자(3바이트) 남아야 함
                if stripped.len() >= 3 {
                    result = stripped.to_string();
                }
            }
        }
        result
    }

    /// 엔티티 정규화: 이름이 같으면 같은 엔티티 반환
    fn resolve_entity(&mut self, name: &str) -> Entity {
        if let Some(entity) = self.entity_cache.get(name) {
            return entity.clone();
        }

        let entity_type = self.infer_entity_type(name);
        let entity = match entity_type {
            "Person" => Entity::person(name),
            "Organization" => Entity::organization(name),
            "Concept" => Entity::concept(name),
            "Event" => Entity::event(name),
            "Location" => Entity {
                id: format!("loc_{}", name),
                name: name.to_string(),
                entity_type: EntityType::Location,
                attributes: HashMap::new(),
            },
            _ => Entity {
                id: format!("unk_{}", name),
                name: name.to_string(),
                entity_type: EntityType::Unknown,
                attributes: HashMap::new(),
            },
        };

        self.entity_cache.insert(name.to_string(), entity.clone());
        entity
    }

    /// 엔티티 유형 추론
    fn infer_entity_type(&self, name: &str) -> &'static str {
        // 대명사 체크
        let pronouns = pronoun_entity_type();
        if let Some(t) = pronouns.get(name) {
            return t;
        }

        // 키워드 매칭
        let keywords = entity_type_keywords();
        if let Some(t) = keywords.get(name) {
            return t;
        }

        // 기본값: Unknown
        "Unknown"
    }

    /// 간단한 토큰화 (Phase 1: 공백 기반, Phase 2에서 lindera로 교체)
    fn tokenize(&self, text: &str) -> Vec<Token> {
        text.split_whitespace()
            .map(|s| Token {
                text: s.to_string(),
            })
            .collect()
    }

    /// 문장 분리
    ///
    /// '요', '다' 등 한국어 종결어미는 단어 끝(뒤에 공백이나 문장 끝)에서만
    /// 문장 경계로 인식한다. "요즘", "다시" 등 단어 중간의 '요'/'다'로
    /// 잘못 분리되는 문제를 방지한다.
    fn split_sentences(&self, text: &str) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = text.chars().collect();

        for (i, &ch) in chars.iter().enumerate() {
            current.push(ch);

            // 구두점은 무조건 문장 경계
            let is_punctuation = ch == '.' || ch == '!' || ch == '?';

            // 한국어 종결어미는 단어 끝(뒤가 공백이거나 문장 끝)에서만 경계
            let is_korean_ending = (ch == '요' || ch == '다')
                && (i + 1 >= chars.len() || chars[i + 1].is_whitespace()
                    || chars[i + 1] == '.' || chars[i + 1] == '!'
                    || chars[i + 1] == '?' || chars[i + 1] == ',');

            if is_punctuation || is_korean_ending {
                let trimmed = current.trim().to_string();
                if trimmed.len() > 2 {
                    sentences.push(trimmed);
                }
                current = String::new();
            }
        }

        // 남은 부분
        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() && trimmed.len() > 1 {
            sentences.push(trimmed);
        }

        // 문장 분리가 안 되면 전체를 하나의 문장으로
        if sentences.is_empty() {
            sentences.push(text.to_string());
        }

        sentences
    }

    /// 복문을 절(clause) 단위로 분리
    ///
    /// "동료들은 좋은데 분위기가 안 좋아"
    /// → ["동료들은 좋은데", "분위기가 안 좋아"]
    ///
    /// 접속어미("~는데", "~지만", "~고")가 포함된 토큰을 찾아
    /// 해당 지점에서 절을 분리한다.
    fn split_clauses(&self, sentence: &str) -> Vec<String> {
        let tokens = self.tokenize(sentence);
        if tokens.len() < 3 {
            // 토큰이 적으면 분리 불필요
            return vec![sentence.to_string()];
        }

        let mut clauses = Vec::new();
        let mut current_tokens: Vec<&str> = Vec::new();

        for (i, token) in tokens.iter().enumerate() {
            current_tokens.push(&token.text);

            // 마지막 토큰이면 분리하지 않음
            if i == tokens.len() - 1 {
                continue;
            }

            // 접속어미로 끝나는 토큰 찾기
            let has_conjunctive = CONJUNCTIVE_ENDINGS.iter().any(|ending| {
                token.text.ends_with(ending)
            });

            if has_conjunctive && current_tokens.len() >= 2 {
                // 뒤에 최소 1개 토큰(술어)이 남아있어야 분리
                let remaining = tokens.len() - i - 1;
                if remaining >= 1 {
                    let clause = current_tokens.join(" ");
                    clauses.push(clause);
                    current_tokens.clear();
                }
            }
        }

        // 남은 토큰
        if !current_tokens.is_empty() {
            let clause = current_tokens.join(" ");
            if !clause.trim().is_empty() {
                clauses.push(clause);
            }
        }

        if clauses.is_empty() {
            clauses.push(sentence.to_string());
        }

        if clauses.len() > 1 {
            debug!(
                sentence = sentence,
                clauses = ?clauses,
                "복문 → {}개 절 분리", clauses.len()
            );
        }

        clauses
    }

    /// 동사 정규화 (어미 제거 → 기본형)
    fn normalize_verb(&self, token: &str) -> String {
        // 긴 어미부터 매칭 (우선순위)
        let endings = &[
            // 복합 어미
            "합니다", "합니까", "했습니다",
            "중이야", "중이다",
            "이야", "해요", "해",
            "합니", "는다", "ㄴ다", "었다", "였다",
            "겠다", "이다",
            // 관형 + 접속
            "은데", "는데", "지만",
            // 의문/추측
            "을까", "ㄹ까", "까",
            // 단순 어미
            "어", "아", "여",
        ];

        for ending in endings {
            if token.ends_with(ending) {
                let stem = token.trim_end_matches(ending);
                if !stem.is_empty() {
                    return stem.to_string();
                }
            }
        }

        // 어미 제거 실패 시 원형 반환
        token.to_string()
    }

    /// 한국어 받침(종성) 제거를 통한 어간 복원
    /// "둘" (ㄷ+ㅜ+ㄹ) → "두" (ㄷ+ㅜ)
    /// 규칙: ㄹ 받침이 있는 글자에서 받침을 제거하여 원형 복원
    fn remove_final_consonant(&self, ch: char) -> Option<char> {
        let code = ch as u32;
        // 한글 유니코드 범위: 0xAC00 ~ 0xD7A3
        if !(0xAC00..=0xD7A3).contains(&code) {
            return None;
        }
        let offset = code - 0xAC00;
        let jongseong = offset % 28;
        if jongseong == 0 {
            return None; // 이미 받침 없음
        }
        // 받침 제거
        let without_jong = code - jongseong;
        char::from_u32(without_jong)
    }

    /// SPO 추출 신뢰도 계산
    fn calculate_confidence(&self, subject: &Entity, _predicate: &Predicate, object: &Entity) -> f32 {
        let mut score: f32 = 0.5;

        // 알려진 엔티티 유형이면 +0.2
        if subject.entity_type != EntityType::Unknown {
            score += 0.15;
        }
        if object.entity_type != EntityType::Unknown {
            score += 0.15;
        }

        // 캐시에 있는 엔티티면 +0.1 (이전에 등장)
        if self.entity_cache.contains_key(&subject.name) {
            score += 0.1;
        }

        score.min(1.0)
    }
}

#[derive(Clone, Debug)]
struct Token {
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_spo_extraction() {
        let mut extractor = SPOExtractor::new();
        let session_id = Uuid::new_v4();

        let triples = extractor.extract("나는 회사가 힘들어", "speaker_01", session_id);
        assert!(!triples.is_empty(), "트리플이 추출되어야 함");

        let t = &triples[0];
        assert_eq!(t.subject.name, "나");
        assert_eq!(t.predicate.polarity, Polarity::Negative);
    }

    #[test]
    fn test_polarity_detection() {
        let mut extractor = SPOExtractor::new();
        let session_id = Uuid::new_v4();

        // 부정
        let triples = extractor.extract("나는 요즘 너무 힘들어", "speaker_01", session_id);
        if let Some(t) = triples.first() {
            assert_eq!(t.predicate.polarity, Polarity::Negative);
            assert!(t.predicate.intensity > 0.8, "너무 → 높은 강도");
        }
    }

    #[test]
    fn test_subject_inheritance() {
        let mut extractor = SPOExtractor::new();
        let session_id = Uuid::new_v4();

        // 첫 문장에서 주어 설정
        extractor.extract("나는 회사에 간다", "speaker_01", session_id);

        // 주어 생략된 문장
        let triples = extractor.extract("밥을 먹었다", "speaker_01", session_id);
        if let Some(t) = triples.first() {
            assert_eq!(t.subject.name, "나", "직전 주어 '나'가 승계되어야 함");
        }
    }

    #[test]
    fn test_compound_sentence_splitting() {
        let mut extractor = SPOExtractor::new();
        let session_id = Uuid::new_v4();

        // "동료들은 좋은데 분위기가 안 좋아" → 2개 트리플
        let triples = extractor.extract(
            "동료들은 좋은데 분위기가 안 좋아",
            "speaker_01",
            session_id,
        );
        assert_eq!(triples.len(), 2, "복문에서 2개 트리플 추출");

        // 첫 번째: 동료 → 좋다 / Positive
        assert_eq!(triples[0].subject.name, "동료");
        assert_eq!(triples[0].predicate.polarity, Polarity::Positive);

        // 두 번째: 분위기 → 안좋다 / Negative
        assert_eq!(triples[1].subject.name, "분위기");
        assert_eq!(triples[1].predicate.polarity, Polarity::Negative);
    }

    #[test]
    fn test_negation_binding() {
        let mut extractor = SPOExtractor::new();
        let session_id = Uuid::new_v4();

        let triples = extractor.extract("분위기가 안 좋아", "speaker_01", session_id);
        assert!(!triples.is_empty());

        let t = &triples[0];
        assert_eq!(t.predicate.polarity, Polarity::Negative,
            "안 + 좋아 → Negative");
        assert!(t.predicate.verb.contains("안"),
            "술어에 부정어 결합: {}", t.predicate.verb);
    }

    #[test]
    fn test_compound_predicate() {
        let mut extractor = SPOExtractor::new();
        let session_id = Uuid::new_v4();

        // "그만둘까 생각중이야" 에서 주어 승계를 위해 먼저 주어 설정
        extractor.extract("나는 회사가 힘들어", "speaker_01", session_id);

        let triples = extractor.extract(
            "회사를 그만둘까 생각중이야",
            "speaker_01",
            session_id,
        );
        assert!(!triples.is_empty());

        let t = &triples[0];
        // "그만두"가 핵심 술어
        assert!(t.predicate.verb.contains("그만두"),
            "복합 술어에서 핵심 동사 추출: {}", t.predicate.verb);
        assert_eq!(t.predicate.polarity, Polarity::Negative,
            "그만두다 → Negative");
    }

    #[test]
    fn test_plural_suffix_stripping() {
        let mut extractor = SPOExtractor::new();
        let session_id = Uuid::new_v4();

        let triples = extractor.extract("동료들은 좋아", "speaker_01", session_id);
        assert!(!triples.is_empty());
        assert_eq!(triples[0].subject.name, "동료",
            "'들' 접미사 제거: 동료들 → 동료");
    }
}
