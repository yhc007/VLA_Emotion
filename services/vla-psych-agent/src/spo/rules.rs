use std::collections::HashMap;

/// 한국어 SPO 추출을 위한 규칙 정의
///
/// 한국어는 SOV 어순이므로 조사(particle)를 기반으로
/// 주어/목적어/술어를 판별한다.

/// 주어 표지 조사
pub const SUBJECT_PARTICLES: &[&str] = &[
    "은", "는", "이", "가",
    "에서",  // 단체 주어: "회사에서"
    "께서",  // 존칭: "선생님께서"
];

/// 목적어 표지 조사
pub const OBJECT_PARTICLES: &[&str] = &[
    "을", "를",
    "에게", "한테", "에게서", "한테서",
    "에",    // 장소/대상: "학교에"
    "로", "으로",
];

/// 부정 표현 (Polarity 판별)
pub const NEGATIVE_MARKERS: &[&str] = &[
    "않", "안", "못", "없", "싫", "힘들", "어렵",
    "나쁘", "슬프", "무섭", "짜증", "화나", "불안",
    "걱정", "두렵", "괴롭", "지치", "피곤",
    "무시", "미워", "답답", "억울", "외롭", "서운",
    "그만두", "포기", "후회", "실망", "속상",
];

/// 긍정 표현
pub const POSITIVE_MARKERS: &[&str] = &[
    "좋", "행복", "즐겁", "기쁘", "편안", "만족",
    "감사", "사랑", "희망", "기대", "설레",
    "뿌듯", "든든", "자랑",
    "괜찮", "다행", "재밌", "신나", "즐거",
    "고맙", "기특", "대견",
];

/// 접속어미 (복문 분리용)
/// 앞 절과 뒤 절을 나누는 연결 어미
pub const CONJUNCTIVE_ENDINGS: &[&str] = &[
    "는데", "은데",          // 대조/배경: "좋은데 ~"
    "지만", "만",            // 대조: "좋지만 ~"
    "고",                    // 나열: "좋고 ~"
    "면서", "으면서",        // 동시: "웃으면서 ~"
    "니까", "으니까",        // 이유: "힘드니까 ~"
    "어서", "아서",          // 순서/이유: "만나서 ~"
];

/// 부정 부사 (술어에 결합)
pub const NEGATION_ADVERBS: &[&str] = &[
    "안", "못",
];

/// 복합 술어 패턴: "X할까/하려고 생각중" 등에서 X를 핵심 술어로 추출
/// (보조 용언 패턴)
pub const AUXILIARY_VERB_MARKERS: &[&str] = &[
    "생각중", "생각", "고민중", "고민",
    "계획", "예정", "준비",
];

/// 복합 술어 연결 어미: "~ㄹ까", "~하려고", "~하고 싶" 등
pub const COMPOUND_PRED_CONNECTORS: &[&str] = &[
    "ㄹ까", "을까", "울까",     // 의지/추측: "그만둘까"
    "려고", "하려고",           // 의도: "하려고"
    "고 싶", "고싶",           // 희망: "하고 싶"
];

/// 강도 부사 (Intensity 증폭)
pub const INTENSIFIERS: &[(&str, f32)] = &[
    ("너무", 0.9),
    ("매우", 0.85),
    ("정말", 0.85),
    ("진짜", 0.85),
    ("아주", 0.8),
    ("엄청", 0.9),
    ("완전", 0.95),
    ("굉장히", 0.85),
    ("상당히", 0.7),
    ("약간", 0.3),
    ("조금", 0.3),
    ("좀", 0.3),
    ("살짝", 0.2),
];

/// 대명사 → 엔티티 유형 매핑
pub fn pronoun_entity_type() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    // 1인칭
    map.insert("나", "Person");
    map.insert("저", "Person");
    map.insert("내", "Person");
    map.insert("제", "Person");
    map.insert("우리", "Person");
    // 2인칭
    map.insert("너", "Person");
    map.insert("당신", "Person");
    // 3인칭
    map.insert("그", "Person");
    map.insert("그녀", "Person");
    map.insert("그들", "Person");
    // 지시
    map.insert("이것", "Object");
    map.insert("그것", "Object");
    map.insert("저것", "Object");
    map.insert("여기", "Location");
    map.insert("거기", "Location");
    map.insert("저기", "Location");
    map
}

/// 엔티티 유형 추론을 위한 키워드
pub fn entity_type_keywords() -> HashMap<&'static str, &'static str> {
    let mut map = HashMap::new();
    // Organization
    for kw in &["회사", "학교", "병원", "정부", "기관", "부서", "팀", "조직"] {
        map.insert(*kw, "Organization");
    }
    // Concept
    for kw in &["스트레스", "행복", "우울", "불안", "사랑", "돈", "시간", "자유", "성공", "실패"] {
        map.insert(*kw, "Concept");
    }
    // Event
    for kw in &["회의", "퇴사", "입사", "결혼", "이사", "여행", "시험", "면접", "출장"] {
        map.insert(*kw, "Event");
    }
    // Location
    for kw in &["집", "사무실", "서울", "부산", "해외", "나라"] {
        map.insert(*kw, "Location");
    }
    // Person (역할/관계)
    for kw in &[
        "상사", "동료", "친구", "선배", "후배", "부하",
        "아버지", "어머니", "아빠", "엄마", "형", "누나", "오빠", "언니",
        "남편", "아내", "아들", "딸", "할아버지", "할머니",
        "선생님", "교수", "의사", "간호사", "사장", "팀장", "부장", "과장",
    ] {
        map.insert(*kw, "Person");
    }
    map
}
