use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// FACS Action Units - 얼굴 미세표정 단위
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ActionUnits {
    // Upper Face
    pub au1_inner_brow_raiser: f32,   // 눈썹 안쪽 올림
    pub au2_outer_brow_raiser: f32,   // 눈썹 바깥쪽 올림
    pub au4_brow_lowerer: f32,        // 눈썹 내림 (찡그림)
    pub au5_upper_lid_raiser: f32,    // 윗눈꺼풀 올림 (놀람)
    pub au6_cheek_raiser: f32,        // 볼 올림 (진짜 미소)
    pub au7_lid_tightener: f32,       // 눈꺼풀 조임
    
    // Lower Face
    pub au9_nose_wrinkler: f32,       // 코 찡그림 (혐오)
    pub au10_upper_lip_raiser: f32,   // 윗입술 올림
    pub au12_lip_corner_puller: f32,  // 입꼬리 올림 (미소)
    pub au14_dimpler: f32,            // 보조개
    pub au15_lip_corner_depressor: f32, // 입꼬리 내림 (슬픔)
    pub au17_chin_raiser: f32,        // 턱 올림
    pub au20_lip_stretcher: f32,      // 입술 늘림 (두려움)
    pub au23_lip_tightener: f32,      // 입술 조임
    pub au24_lip_pressor: f32,        // 입술 누름
    pub au25_lips_part: f32,          // 입 벌림
    pub au26_jaw_drop: f32,           // 턱 벌림 (놀람)
    pub au28_lip_suck: f32,           // 입술 빨기
}

/// 감정 상태 (AU 조합으로 추론)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EmotionState {
    pub happiness: f32,    // AU6 + AU12
    pub sadness: f32,      // AU1 + AU4 + AU15
    pub anger: f32,        // AU4 + AU5 + AU7 + AU23
    pub fear: f32,         // AU1 + AU2 + AU4 + AU5 + AU20 + AU26
    pub surprise: f32,     // AU1 + AU2 + AU5 + AU26
    pub disgust: f32,      // AU9 + AU15 + AU17
    pub contempt: f32,     // AU12 (한쪽만) + AU14
    pub neutral: f32,
    
    pub valence: f32,      // 긍정(-1) ~ 부정(+1)
    pub arousal: f32,      // 각성 수준 0~1
    pub dominance: f32,    // 지배/통제감 0~1
}

impl Default for EmotionState {
    fn default() -> Self {
        Self {
            happiness: 0.0,
            sadness: 0.0,
            anger: 0.0,
            fear: 0.0,
            surprise: 0.0,
            disgust: 0.0,
            contempt: 0.0,
            neutral: 1.0,
            valence: 0.0,
            arousal: 0.0,
            dominance: 0.5,
        }
    }
}

/// MediaPipe Face Mesh 키 포인트 인덱스
pub mod landmark_indices {
    // 눈썹
    pub const LEFT_EYEBROW_INNER: usize = 105;
    pub const LEFT_EYEBROW_OUTER: usize = 70;
    pub const RIGHT_EYEBROW_INNER: usize = 334;
    pub const RIGHT_EYEBROW_OUTER: usize = 300;
    
    // 눈
    pub const LEFT_EYE_UPPER: usize = 159;
    pub const LEFT_EYE_LOWER: usize = 145;
    pub const LEFT_EYE_INNER: usize = 133;
    pub const LEFT_EYE_OUTER: usize = 33;
    pub const RIGHT_EYE_UPPER: usize = 386;
    pub const RIGHT_EYE_LOWER: usize = 374;
    pub const RIGHT_EYE_INNER: usize = 362;
    pub const RIGHT_EYE_OUTER: usize = 263;
    
    // 코
    pub const NOSE_TIP: usize = 1;
    pub const NOSE_LEFT: usize = 129;
    pub const NOSE_RIGHT: usize = 358;
    
    // 입
    pub const MOUTH_LEFT: usize = 61;
    pub const MOUTH_RIGHT: usize = 291;
    pub const MOUTH_UPPER: usize = 0;
    pub const MOUTH_LOWER: usize = 17;
    pub const UPPER_LIP_TOP: usize = 13;
    pub const LOWER_LIP_BOTTOM: usize = 14;
    
    // 턱/볼
    pub const CHIN: usize = 152;
    pub const LEFT_CHEEK: usize = 234;
    pub const RIGHT_CHEEK: usize = 454;
    
    // 기준점
    pub const FOREHEAD: usize = 10;
}

/// 랜드마크에서 AU 계산
pub fn compute_action_units(landmarks: &[[f32; 3]; 468], baseline: Option<&[[f32; 3]; 468]>) -> ActionUnits {
    let mut au = ActionUnits::default();
    
    // 기준선 (중립 표정) 사용 또는 일반 비율 사용
    let base = baseline.unwrap_or(landmarks);
    
    // 얼굴 크기 정규화를 위한 기준 거리 (눈 사이 거리)
    let eye_dist = distance_3d(
        &landmarks[landmark_indices::LEFT_EYE_INNER],
        &landmarks[landmark_indices::RIGHT_EYE_INNER],
    );
    
    if eye_dist < 0.001 {
        return au; // 유효하지 않은 데이터
    }
    
    // === AU1: Inner Brow Raiser (눈썹 안쪽 올림) ===
    let left_inner_brow_rise = base[landmark_indices::LEFT_EYEBROW_INNER][1] 
        - landmarks[landmark_indices::LEFT_EYEBROW_INNER][1];
    let right_inner_brow_rise = base[landmark_indices::RIGHT_EYEBROW_INNER][1] 
        - landmarks[landmark_indices::RIGHT_EYEBROW_INNER][1];
    au.au1_inner_brow_raiser = ((left_inner_brow_rise + right_inner_brow_rise) / 2.0 / eye_dist * 10.0)
        .clamp(0.0, 1.0);
    
    // === AU2: Outer Brow Raiser (눈썹 바깥쪽 올림) ===
    let left_outer_brow_rise = base[landmark_indices::LEFT_EYEBROW_OUTER][1] 
        - landmarks[landmark_indices::LEFT_EYEBROW_OUTER][1];
    let right_outer_brow_rise = base[landmark_indices::RIGHT_EYEBROW_OUTER][1] 
        - landmarks[landmark_indices::RIGHT_EYEBROW_OUTER][1];
    au.au2_outer_brow_raiser = ((left_outer_brow_rise + right_outer_brow_rise) / 2.0 / eye_dist * 10.0)
        .clamp(0.0, 1.0);
    
    // === AU4: Brow Lowerer (눈썹 내림/찡그림) ===
    let brow_lower = landmarks[landmark_indices::LEFT_EYEBROW_INNER][1] 
        - base[landmark_indices::LEFT_EYEBROW_INNER][1];
    au.au4_brow_lowerer = (brow_lower / eye_dist * 10.0).clamp(0.0, 1.0);
    
    // === AU5: Upper Lid Raiser (윗눈꺼풀 올림) ===
    let left_eye_open = distance_3d(
        &landmarks[landmark_indices::LEFT_EYE_UPPER],
        &landmarks[landmark_indices::LEFT_EYE_LOWER],
    );
    let right_eye_open = distance_3d(
        &landmarks[landmark_indices::RIGHT_EYE_UPPER],
        &landmarks[landmark_indices::RIGHT_EYE_LOWER],
    );
    let avg_eye_open = (left_eye_open + right_eye_open) / 2.0;
    au.au5_upper_lid_raiser = (avg_eye_open / eye_dist * 5.0 - 0.5).clamp(0.0, 1.0);
    
    // === AU6: Cheek Raiser (볼 올림 - 진정한 미소) ===
    let left_cheek_rise = base[landmark_indices::LEFT_CHEEK][1] 
        - landmarks[landmark_indices::LEFT_CHEEK][1];
    let right_cheek_rise = base[landmark_indices::RIGHT_CHEEK][1] 
        - landmarks[landmark_indices::RIGHT_CHEEK][1];
    au.au6_cheek_raiser = ((left_cheek_rise + right_cheek_rise) / 2.0 / eye_dist * 10.0)
        .clamp(0.0, 1.0);
    
    // === AU12: Lip Corner Puller (입꼬리 올림 - 미소) ===
    let mouth_width = distance_3d(
        &landmarks[landmark_indices::MOUTH_LEFT],
        &landmarks[landmark_indices::MOUTH_RIGHT],
    );
    let base_mouth_width = distance_3d(
        &base[landmark_indices::MOUTH_LEFT],
        &base[landmark_indices::MOUTH_RIGHT],
    );
    let left_corner_rise = base[landmark_indices::MOUTH_LEFT][1] 
        - landmarks[landmark_indices::MOUTH_LEFT][1];
    let right_corner_rise = base[landmark_indices::MOUTH_RIGHT][1] 
        - landmarks[landmark_indices::MOUTH_RIGHT][1];
    au.au12_lip_corner_puller = ((left_corner_rise + right_corner_rise) / 2.0 / eye_dist * 10.0 
        + (mouth_width - base_mouth_width) / eye_dist * 5.0)
        .clamp(0.0, 1.0);
    
    // === AU15: Lip Corner Depressor (입꼬리 내림 - 슬픔) ===
    au.au15_lip_corner_depressor = ((-left_corner_rise - right_corner_rise) / 2.0 / eye_dist * 10.0)
        .clamp(0.0, 1.0);
    
    // === AU25: Lips Part (입 벌림) ===
    let lip_distance = distance_3d(
        &landmarks[landmark_indices::UPPER_LIP_TOP],
        &landmarks[landmark_indices::LOWER_LIP_BOTTOM],
    );
    au.au25_lips_part = (lip_distance / eye_dist * 3.0).clamp(0.0, 1.0);
    
    // === AU26: Jaw Drop (턱 벌림) ===
    let jaw_drop = distance_3d(
        &landmarks[landmark_indices::NOSE_TIP],
        &landmarks[landmark_indices::CHIN],
    );
    let base_jaw = distance_3d(
        &base[landmark_indices::NOSE_TIP],
        &base[landmark_indices::CHIN],
    );
    au.au26_jaw_drop = ((jaw_drop - base_jaw) / eye_dist * 5.0).clamp(0.0, 1.0);
    
    // === AU9: Nose Wrinkler (코 찡그림) ===
    let nose_scrunch = distance_3d(
        &landmarks[landmark_indices::NOSE_LEFT],
        &landmarks[landmark_indices::NOSE_RIGHT],
    );
    let base_nose = distance_3d(
        &base[landmark_indices::NOSE_LEFT],
        &base[landmark_indices::NOSE_RIGHT],
    );
    au.au9_nose_wrinkler = ((base_nose - nose_scrunch) / eye_dist * 10.0).clamp(0.0, 1.0);
    
    au
}

/// AU에서 감정 상태 추론
pub fn infer_emotion(au: &ActionUnits) -> EmotionState {
    let mut emotion = EmotionState::default();
    
    // 행복 (Duchenne smile): AU6 + AU12
    emotion.happiness = (au.au6_cheek_raiser * 0.4 + au.au12_lip_corner_puller * 0.6)
        .clamp(0.0, 1.0);
    
    // 슬픔: AU1 + AU4 + AU15
    emotion.sadness = (au.au1_inner_brow_raiser * 0.3 + au.au4_brow_lowerer * 0.3 
        + au.au15_lip_corner_depressor * 0.4)
        .clamp(0.0, 1.0);
    
    // 분노: AU4 + AU5 + AU7 + AU23
    emotion.anger = (au.au4_brow_lowerer * 0.4 + au.au5_upper_lid_raiser * 0.2 
        + au.au7_lid_tightener * 0.2 + au.au23_lip_tightener * 0.2)
        .clamp(0.0, 1.0);
    
    // 두려움: AU1 + AU2 + AU4 + AU5 + AU20 + AU26
    emotion.fear = (au.au1_inner_brow_raiser * 0.2 + au.au2_outer_brow_raiser * 0.15 
        + au.au4_brow_lowerer * 0.15 + au.au5_upper_lid_raiser * 0.2 
        + au.au20_lip_stretcher * 0.15 + au.au26_jaw_drop * 0.15)
        .clamp(0.0, 1.0);
    
    // 놀람: AU1 + AU2 + AU5 + AU26
    emotion.surprise = (au.au1_inner_brow_raiser * 0.25 + au.au2_outer_brow_raiser * 0.25 
        + au.au5_upper_lid_raiser * 0.25 + au.au26_jaw_drop * 0.25)
        .clamp(0.0, 1.0);
    
    // 혐오: AU9 + AU15 + AU17
    emotion.disgust = (au.au9_nose_wrinkler * 0.5 + au.au15_lip_corner_depressor * 0.25 
        + au.au17_chin_raiser * 0.25)
        .clamp(0.0, 1.0);
    
    // 중립 (다른 감정이 낮을 때)
    let max_emotion = emotion.happiness
        .max(emotion.sadness)
        .max(emotion.anger)
        .max(emotion.fear)
        .max(emotion.surprise)
        .max(emotion.disgust);
    emotion.neutral = (1.0 - max_emotion).clamp(0.0, 1.0);
    
    // Valence-Arousal-Dominance (VAD) 계산
    // Valence: 긍정(+) vs 부정(-)
    emotion.valence = emotion.happiness - (emotion.sadness + emotion.anger + emotion.fear + emotion.disgust) / 4.0;
    emotion.valence = emotion.valence.clamp(-1.0, 1.0);
    
    // Arousal: 각성 수준
    emotion.arousal = (emotion.anger + emotion.fear + emotion.surprise + emotion.happiness * 0.5) / 3.5;
    emotion.arousal = emotion.arousal.clamp(0.0, 1.0);
    
    // Dominance: 통제감 (분노=높음, 두려움/슬픔=낮음)
    emotion.dominance = 0.5 + emotion.anger * 0.3 - emotion.fear * 0.3 - emotion.sadness * 0.2;
    emotion.dominance = emotion.dominance.clamp(0.0, 1.0);
    
    emotion
}

/// 3D 거리 계산
fn distance_3d(p1: &[f32; 3], p2: &[f32; 3]) -> f32 {
    ((p1[0] - p2[0]).powi(2) + (p1[1] - p2[1]).powi(2) + (p1[2] - p2[2]).powi(2)).sqrt()
}

/// MediaPipe 초기화 및 분석 결과
#[derive(Clone, Debug, Default)]
pub struct FaceAnalysisResult {
    pub landmarks: Option<[[f32; 3]; 468]>,
    pub action_units: ActionUnits,
    pub emotion: EmotionState,
    pub confidence: f32,
}

/// Leptos 컴포넌트: 얼굴 분석 오버레이
#[component]
pub fn FaceAnalysisOverlay(
    emotion: ReadSignal<EmotionState>,
    action_units: ReadSignal<ActionUnits>,
    #[prop(default = true)] show_details: bool,
) -> impl IntoView {
    let valence_positive = move || emotion.get().valence > 0.0;
    
    view! {
        <div class="absolute top-2 left-2 bg-black/70 rounded-lg p-3 text-xs font-mono">
            // 주요 감정 표시
            <div class="mb-2">
                <div class="text-grok-blue font-semibold mb-1">"Emotion"</div>
                {move || {
                    let e = emotion.get();
                    view! {
                        <div class="space-y-0.5">
                            {emotion_bar("😊 Happy", e.happiness)}
                            {emotion_bar("😢 Sad", e.sadness)}
                            {emotion_bar("😠 Angry", e.anger)}
                            {emotion_bar("😨 Fear", e.fear)}
                            {emotion_bar("😲 Surprise", e.surprise)}
                            {emotion_bar("🤢 Disgust", e.disgust)}
                        </div>
                    }
                }}
            </div>
            
            // VAD 표시
            <div class="border-t border-grok-border pt-2 mt-2">
                <div class="flex justify-between text-grok-muted">
                    <span>"Valence"</span>
                    <span style=move || {
                        if valence_positive() { "color: #4ade80" } else { "color: #f87171" }
                    }>
                        {move || format!("{:+.2}", emotion.get().valence)}
                    </span>
                </div>
                <div class="flex justify-between text-grok-muted">
                    <span>"Arousal"</span>
                    <span>{move || format!("{:.2}", emotion.get().arousal)}</span>
                </div>
                <div class="flex justify-between text-grok-muted">
                    <span>"Dominance"</span>
                    <span>{move || format!("{:.2}", emotion.get().dominance)}</span>
                </div>
            </div>
            
            // AU 상세 (선택적)
            <Show when=move || show_details>
                <div class="border-t border-grok-border pt-2 mt-2">
                    <div class="text-grok-blue font-semibold mb-1">"Action Units"</div>
                    {move || {
                        let au = action_units.get();
                        view! {
                            <div class="grid grid-cols-2 gap-x-4 gap-y-0.5 text-grok-muted">
                                <span>"AU1 (Brow↑)"</span>
                                <span>{format!("{:.2}", au.au1_inner_brow_raiser)}</span>
                                <span>"AU4 (Brow↓)"</span>
                                <span>{format!("{:.2}", au.au4_brow_lowerer)}</span>
                                <span>"AU6 (Cheek)"</span>
                                <span>{format!("{:.2}", au.au6_cheek_raiser)}</span>
                                <span>"AU12 (Smile)"</span>
                                <span>{format!("{:.2}", au.au12_lip_corner_puller)}</span>
                                <span>"AU25 (Lips)"</span>
                                <span>{format!("{:.2}", au.au25_lips_part)}</span>
                            </div>
                        }
                    }}
                </div>
            </Show>
        </div>
    }
}

/// 감정 바 뷰 생성 (헬퍼 함수)
fn emotion_bar(label: &'static str, value: f32) -> impl IntoView {
    let width_percent = (value * 100.0).min(100.0);
    view! {
        <div class="flex items-center gap-2">
            <span class="w-20 text-grok-muted">{label}</span>
            <div class="flex-1 h-1.5 bg-grok-border rounded-full overflow-hidden">
                <div 
                    class="h-full bg-grok-blue transition-all duration-200"
                    style=format!("width: {}%", width_percent)
                />
            </div>
            <span class="w-8 text-right">{format!("{:.0}%", width_percent)}</span>
        </div>
    }
}

/// 지배적 감정 텍스트로 반환
pub fn dominant_emotion(emotion: &EmotionState) -> &'static str {
    let emotions = [
        (emotion.happiness, "😊 행복"),
        (emotion.sadness, "😢 슬픔"),
        (emotion.anger, "😠 분노"),
        (emotion.fear, "😨 두려움"),
        (emotion.surprise, "😲 놀람"),
        (emotion.disgust, "🤢 혐오"),
        (emotion.neutral, "😐 중립"),
    ];
    
    emotions.iter()
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .map(|(_, name)| *name)
        .unwrap_or("😐 중립")
}
