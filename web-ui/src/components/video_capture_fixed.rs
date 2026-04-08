// 📝 VLA-Emotion Video Capture 수정안 - 메모리 누수 방지

// 기존 100ms → 500ms로 변경
let handle = gloo_timers::callback::Interval::new(500, move || {
    if is_active.get() {
        // face detection 로직
    }
});

// ✅ cleanup 로직 추가
Effect::new(move |_| {
    if is_active.get() {
        let handle = gloo_timers::callback::Interval::new(500, move || {
            // face detection 로직
        });
        
        // cleanup 함수 반환
        move || {
            handle.forget(); // 명시적 해제
        }
    }
});

// ✅ 에러 핸들링 개선
wasm_bindgen_futures::spawn_local(async move {
    match detectFace(&video).await {
        Ok(result) => {
            // 성공 처리
        }
        Err(e) => {
            log::warn!("Face detection failed, continuing: {:?}", e);
            return; // 에러 시 조용히 종료
        }
    }
});