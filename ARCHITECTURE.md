# VLA MediaSet Architecture

## Overview

VLA (Voice-Language-Analysis)는 고객의 문제를 발견하고 솔루션을 제안하는 AI 기반 분석 시스템입니다.

## Core Concepts

```
┌─────────────────────────────────────────────────────┐
│                      CASE                           │
│  (하나의 고객 문제 해결 세션)                          │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌─────────────────────────────────────────────┐   │
│  │              MEDIA SET                       │   │
│  │  (케이스에 연결된 모든 미디어 묶음)             │   │
│  ├─────────────────────────────────────────────┤   │
│  │                                             │   │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐       │   │
│  │  │  VIDEO  │ │  AUDIO  │ │  DOCS   │       │   │
│  │  │ Stream  │ │ Stream  │ │ Files   │       │   │
│  │  └────┬────┘ └────┬────┘ └────┬────┘       │   │
│  │       │           │           │             │   │
│  │       └───────────┼───────────┘             │   │
│  │                   ▼                         │   │
│  │           ┌─────────────┐                   │   │
│  │           │  TIMELINE   │                   │   │
│  │           │  (동기화)    │                   │   │
│  │           └─────────────┘                   │   │
│  │                   │                         │   │
│  │                   ▼                         │   │
│  │           ┌─────────────┐                   │   │
│  │           │  AI ENGINE  │                   │   │
│  │           │  (Claude)   │                   │   │
│  │           └─────────────┘                   │   │
│  │                   │                         │   │
│  │                   ▼                         │   │
│  │           ┌─────────────┐                   │   │
│  │           │  SOLUTION   │                   │   │
│  │           │   REPORT    │                   │   │
│  │           └─────────────┘                   │   │
│  └─────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

## Data Models

### Case (최상위 컨테이너)

```rust
struct Case {
    id: Uuid,
    client_id: String,
    analyst_id: String,
    status: CaseStatus,  // Draft, Active, Analyzing, Completed
    media_set: MediaSet,
    solution: Option<SolutionReport>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

enum CaseStatus {
    Draft,
    Active,
    Analyzing,
    Completed,
    Archived,
}
```

### MediaSet (미디어 묶음)

```rust
struct MediaSet {
    id: Uuid,
    case_id: Uuid,
    video: Option<VideoStream>,
    audio: Option<AudioStream>,
    documents: Vec<Document>,
    timeline: Timeline,
}
```

### Video Stream

```rust
struct VideoStream {
    id: Uuid,
    url: String,
    duration_ms: u64,
    frames: Vec<FrameAnalysis>,
    status: StreamStatus,
}

struct FrameAnalysis {
    timestamp_ms: u64,
    sentiment: String,
    confidence: f64,
    face_detected: bool,
}
```

### Audio Stream

```rust
struct AudioStream {
    id: Uuid,
    url: String,
    duration_ms: u64,
    transcript: Vec<TranscriptSegment>,
    status: StreamStatus,
}

struct TranscriptSegment {
    id: Uuid,
    start_ms: u64,
    end_ms: u64,
    speaker: Speaker,
    text: String,
    confidence: f64,
}

enum Speaker {
    Client,
    Analyst,
    Unknown,
}
```

### Document

```rust
struct Document {
    id: Uuid,
    filename: String,
    file_type: DocType,
    url: String,
    size_bytes: u64,
    extracted_text: Option<String>,
    mentioned_at: Vec<TimeMarker>,
    uploaded_at: DateTime<Utc>,
}

enum DocType {
    Pdf,
    Image,
    Excel,
    Word,
    Text,
    Other,
}
```

### Timeline (동기화)

```rust
struct Timeline {
    markers: Vec<TimeMarker>,
    events: Vec<TimelineEvent>,
}

struct TimeMarker {
    id: Uuid,
    timestamp_ms: u64,
    media_type: MediaType,
    media_id: Uuid,
    description: String,
}

struct TimelineEvent {
    id: Uuid,
    timestamp_ms: u64,
    event_type: EventType,
    data: serde_json::Value,
}

enum MediaType {
    Video,
    Audio,
    Document,
}

enum EventType {
    SentimentChange,
    DocumentMention,
    KeyInsight,
    ProblemIdentified,
    SolutionProposed,
}
```

### Solution Report

```rust
struct SolutionReport {
    id: Uuid,
    case_id: Uuid,
    problem_statement: String,
    root_causes: Vec<String>,
    priority: Priority,
    solutions: Vec<Solution>,
    action_items: Vec<ActionItem>,
    next_steps: String,
    generated_at: DateTime<Utc>,
}

struct Solution {
    id: Uuid,
    title: String,
    description: String,
    effort: Effort,
    impact: Impact,
}

struct ActionItem {
    id: Uuid,
    description: String,
    assignee: Option<String>,
    due_date: Option<DateTime<Utc>>,
    status: ActionStatus,
}

enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

enum Effort {
    Low,
    Medium,
    High,
}

enum Impact {
    Low,
    Medium,
    High,
}

enum ActionStatus {
    Pending,
    InProgress,
    Completed,
}
```

## API Endpoints

### Case Management

```
POST   /api/v1/case/create              # 새 케이스 생성
GET    /api/v1/case/{id}                # 케이스 조회
PUT    /api/v1/case/{id}                # 케이스 업데이트
DELETE /api/v1/case/{id}                # 케이스 삭제
GET    /api/v1/cases                    # 케이스 목록
```

### Media Management

```
POST   /api/v1/case/{id}/media/video/start    # 비디오 녹화 시작
POST   /api/v1/case/{id}/media/video/stop     # 비디오 녹화 중지
POST   /api/v1/case/{id}/media/document       # 문서 업로드
DELETE /api/v1/case/{id}/media/document/{doc_id}  # 문서 삭제
GET    /api/v1/case/{id}/media-set            # MediaSet 조회
```

### Timeline

```
GET    /api/v1/case/{id}/timeline             # 타임라인 조회
POST   /api/v1/case/{id}/timeline/marker      # 마커 추가
```

### Analysis

```
POST   /api/v1/case/{id}/analyze              # AI 분석 시작
GET    /api/v1/case/{id}/analysis/status      # 분석 상태
GET    /api/v1/case/{id}/solution             # 솔루션 리포트 조회
```

### WebSocket

```
WS     /api/v1/case/{id}/ws                   # 실시간 스트리밍
```

## UI Layout

```
┌──────────────────────────────────────────────────────────┐
│  VLA - Problem Discovery & Solution                      │
├──────────────────────────────────────────────────────────┤
│  [New Case] [Upload Doc] [Start Recording] [Analyze]     │
├─────────────────────────┬────────────────────────────────┤
│                         │                                │
│   VIDEO FEED            │    CONVERSATION LOG            │
│   ┌───────────────┐     │    ┌──────────────────────┐   │
│   │               │     │    │ Client: ...          │   │
│   │      📹       │     │    │ Analyst: ...         │   │
│   │               │     │    │ Client: ...          │   │
│   └───────────────┘     │    └──────────────────────┘   │
│                         │                                │
│   SENTIMENT             │    DOCUMENTS                   │
│   ┌───────────────┐     │    ┌──────────────────────┐   │
│   │ Confused 72%  │     │    │ 📄 requirements.pdf  │   │
│   │ ████████░░    │     │    │ 📊 data.xlsx         │   │
│   └───────────────┘     │    │ 🖼️ screenshot.png    │   │
│                         │    └──────────────────────┘   │
├─────────────────────────┴────────────────────────────────┤
│  TIMELINE                                                │
│  ├──●────────●──────────●─────────●────────●──────────▶ │
│  0:00      0:32       1:15      2:00     3:45           │
│  Start     Doc        Insight   Issue    Solution       │
│            Upload     Found     Found    Proposed       │
└──────────────────────────────────────────────────────────┘
```

## Implementation Phases

### Phase 1: Core Structure ✅
- [x] Basic UI layout
- [x] Video capture
- [x] Audio transcription (Whisper)
- [x] Sentiment analysis (Azure Face)
- [x] AI report generation (Claude)

### Phase 2: MediaSet Integration 🚧
- [ ] MediaSet data model
- [ ] Document upload component
- [ ] Timeline component
- [ ] API extensions

### Phase 3: Enhanced Analysis
- [ ] Multi-media correlation
- [ ] Timestamp synchronization
- [ ] Document content extraction
- [ ] Context-aware AI analysis

### Phase 4: Production Ready
- [ ] Error handling
- [ ] Performance optimization
- [ ] Testing
- [ ] Documentation

---

*Last updated: 2026-04-03*
