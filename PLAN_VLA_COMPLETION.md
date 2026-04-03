# VLA_Emotion 프로젝트 완성 계획

**작성일**: 2026-04-03
**작성자**: Sam 🦊
**목표**: vla-psych-agent를 Production-ready 서비스로 완성

---

## 📊 현재 상태 분석

### ✅ 완성된 부분
| 모듈 | 상태 | 설명 |
|------|------|------|
| SPO 추출 | ✅ 완료 | 룰 기반 Subject-Predicate-Object 추출 |
| 시맨틱 그래프 | ✅ 완료 | petgraph 기반 관계 그래프 |
| 심리 분석 | ✅ 완료 | 상태/추세/이상 탐지 |
| 감정 타입 | ✅ 완료 | 7가지 기본 감정 + 중립 |
| 이벤트 시스템 | ✅ 완료 | pekko-agent-events 연동 |
| AgentActor | ✅ 완료 | 트레이트 구현 (reason/act/respond) |

### ⚠️ 미완성 부분 (Warning 21개 원인)
- Dead code: 구현됐지만 main.rs에서 사용 안 됨
- HTTP API 없음 (현재 데모 모드만)
- 실시간 입력 처리 없음

### 📈 코드 통계
- **총 라인**: 2,885줄
- **서비스**: vla-psych-agent
- **빌드**: ✅ 성공 (warning 21개)

---

## 🎯 완성 목표

### Phase 1: HTTP API 서버 (1-2일)
REST API로 외부에서 VLA 파이프라인 호출 가능하게

```
POST /api/v1/session/start     → 세션 시작
POST /api/v1/analyze/text      → 텍스트 + 감정 분석
GET  /api/v1/session/{id}/state → 현재 심리 상태
GET  /api/v1/session/{id}/graph → 그래프 데이터
WS   /ws/stream                 → 실시간 스트리밍
```

**구현 파일**:
```
services/vla-psych-agent/src/
├── api/
│   ├── mod.rs
│   ├── routes.rs      # Axum 라우터
│   ├── handlers.rs    # 핸들러 함수
│   └── dto.rs         # Request/Response DTO
└── main.rs            # API 서버 모드 추가
```

### Phase 2: WebSocket 실시간 스트리밍 (1일)
클라이언트가 연속 입력 → 실시간 분석 결과 푸시

**기능**:
- 세션별 WebSocket 연결
- 텍스트/감정 입력 스트림
- 분석 결과 실시간 브로드캐스트
- 이상 탐지 시 즉시 알림

### Phase 3: 음성 입력 처리 (2-3일)
음성 → 텍스트 변환 (Whisper 연동)

**옵션**:
1. **로컬 Whisper** (whisper.cpp / whisper-rs)
   - MLX 가속 (Apple Silicon)
   - 오프라인 가능
2. **OpenAI Whisper API**
   - 빠른 구현
   - API 비용 발생

**추천**: Phase 1에서는 OpenAI API, 추후 로컬 전환

### Phase 4: 영상/표정 분석 연동 (2-3일)
표정 인식 → EmotionResult 생성

**옵션**:
1. **fer2013 모델** (로컬 추론)
2. **Azure Face API** / **AWS Rekognition**
3. **MediaPipe Face** (Google, 무료)

**추천**: MediaPipe + 경량 분류기

### Phase 5: Claude LLM 연동 (1-2일)
룰 기반 → AI 기반 심리 분석 강화

**활용**:
- SPO 추출 보조 (애매한 문장 해석)
- 심리 상태 종합 리포트 생성
- 상담사 추천 멘트 제안

**구현**:
```rust
// pekko-agent-llm 활용
let llm = ClaudeClient::new(&config)?;
let analysis = llm.analyze_psych_state(&graph, &history).await?;
```

### Phase 6: 웹 UI (2-3일)
실시간 대시보드 + 그래프 시각화

**스택**: Astro + D3.js (또는 vis.js)

**페이지**:
- `/` - 세션 목록
- `/session/{id}` - 실시간 분석 대시보드
- `/session/{id}/graph` - 인터랙티브 그래프

### Phase 7: 테스트 & 문서화 (1-2일)
- 단위 테스트 (spo, graph, psych)
- 통합 테스트 (API 엔드포인트)
- README 업데이트
- API 문서 (OpenAPI/Swagger)

---

## 📋 상세 구현 계획

### Phase 1 상세: HTTP API

#### 1.1 프로젝트 구조 변경
```bash
# api 모듈 추가
mkdir -p services/vla-psych-agent/src/api
```

#### 1.2 Cargo.toml 의존성 확인
```toml
# 이미 있음
axum = { workspace = true, features = ["ws"] }
tower.workspace = true
tower-http.workspace = true
```

#### 1.3 routes.rs 구현
```rust
use axum::{Router, routing::{get, post}};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/session/start", post(start_session))
        .route("/api/v1/analyze/text", post(analyze_text))
        .route("/api/v1/session/:id/state", get(get_state))
        .route("/api/v1/session/:id/graph", get(get_graph))
        .route("/ws/stream", get(ws_handler))
        .with_state(state)
}
```

#### 1.4 AppState 설계
```rust
#[derive(Clone)]
pub struct AppState {
    sessions: Arc<RwLock<HashMap<Uuid, VlaPsychAgent>>>,
    event_publisher: Arc<EventPublisher>,
}
```

#### 1.5 main.rs 모드 분리
```rust
enum RunMode {
    Demo,      // 현재 동작 (--demo)
    Server,    // HTTP 서버 (기본)
}
```

---

## 🚀 실행 순서

### 즉시 (오늘)
1. [x] 프로젝트 분석 완료
2. [ ] api/ 모듈 생성
3. [ ] 기본 라우터 + health 엔드포인트
4. [ ] `/api/v1/session/start` 구현

### 이번 주
5. [ ] 나머지 REST 엔드포인트
6. [ ] WebSocket 핸들러
7. [ ] 기본 테스트

### 다음 주
8. [ ] 음성 입력 (Whisper)
9. [ ] 표정 분석 연동
10. [ ] Claude LLM 연동

### 이후
11. [ ] 웹 UI
12. [ ] 문서화
13. [ ] 배포 (Docker/Cloudflare)

---

## 📁 최종 프로젝트 구조

```
services/vla-psych-agent/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI + 서버 모드
│   ├── lib.rs
│   ├── api/
│   │   ├── mod.rs
│   │   ├── routes.rs
│   │   ├── handlers.rs
│   │   ├── dto.rs
│   │   └── websocket.rs
│   ├── actors/
│   │   ├── mod.rs
│   │   ├── vla_agent.rs
│   │   └── psych_analyzer.rs
│   ├── graph/
│   │   ├── mod.rs
│   │   └── engine.rs
│   ├── spo/
│   │   ├── mod.rs
│   │   ├── extractor.rs
│   │   └── rules.rs
│   ├── types/
│   │   ├── mod.rs
│   │   ├── emotion.rs
│   │   ├── events.rs
│   │   ├── graph.rs
│   │   ├── psych.rs
│   │   └── spo.rs
│   ├── speech/              # 새로 추가
│   │   ├── mod.rs
│   │   └── whisper.rs
│   └── vision/              # 새로 추가
│       ├── mod.rs
│       └── face_emotion.rs
└── tests/
    ├── api_tests.rs
    └── integration_tests.rs
```

---

## ✅ 성공 기준

- [ ] `cargo run` → HTTP 서버 시작 (포트 8080)
- [ ] `cargo run -- --demo` → 기존 데모 동작
- [ ] REST API 6개 엔드포인트 작동
- [ ] WebSocket 실시간 스트리밍
- [ ] 테스트 커버리지 > 60%
- [ ] Warning 0개
- [ ] README에 API 문서

---

## 🔗 참고 자료

- [pekko-agent-core](../crates/pekko-agent-core) - AgentActor 트레이트
- [pekko-agent-llm](../crates/pekko-agent-llm) - Claude 연동
- [Axum WebSocket](https://docs.rs/axum/latest/axum/extract/ws/)
- [whisper-rs](https://github.com/tazz4843/whisper-rs)

---

*Plan 생성: Sam 🦊 | 2026-04-03*
