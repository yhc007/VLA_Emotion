/**
 * MediaPipe Face Mesh Integration (Vision Tasks API)
 * 468 랜드마크 실시간 감지
 */

let faceLandmarker = null;
let lastResults = null;
let isInitializing = false;

// MediaPipe Vision 초기화
window.initFaceMesh = async function() {
    if (faceLandmarker || isInitializing) return;
    isInitializing = true;
    
    console.log('🦊 Loading MediaPipe Vision...');
    
    try {
        // MediaPipe Vision Tasks CDN
        const vision = await import('https://cdn.jsdelivr.net/npm/@mediapipe/tasks-vision@0.10.8/+esm');
        const { FaceLandmarker, FilesetResolver } = vision;
        
        console.log('🦊 Creating FilesetResolver...');
        const filesetResolver = await FilesetResolver.forVisionTasks(
            'https://cdn.jsdelivr.net/npm/@mediapipe/tasks-vision@0.10.8/wasm'
        );
        
        console.log('🦊 Creating FaceLandmarker...');
        faceLandmarker = await FaceLandmarker.createFromOptions(filesetResolver, {
            baseOptions: {
                modelAssetPath: 'https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/1/face_landmarker.task',
                delegate: 'CPU'
            },
            outputFaceBlendshapes: true,
            runningMode: 'VIDEO',
            numFaces: 1
        });
        
        console.log('🦊 MediaPipe FaceLandmarker initialized!');
        isInitializing = false;
    } catch (e) {
        console.warn('⚠️ MediaPipe init failed, continuing without face detection:', e);
        isInitializing = false;
        // 에러를 throw하지 않고 graceful degradation
        return;
    }
};

// 비디오에서 얼굴 감지
window.detectFace = async function(videoElement) {
    if (!faceLandmarker) {
        try {
            await window.initFaceMesh();
        } catch (e) {
            return null;
        }
    }
    
    if (!faceLandmarker) return null;
    if (!videoElement || videoElement.readyState < 2) return null;
    
    try {
        const startTimeMs = performance.now();
        const results = faceLandmarker.detectForVideo(videoElement, startTimeMs);
        
        if (!results || !results.faceLandmarks || results.faceLandmarks.length === 0) {
            return null;
        }
        
        lastResults = results;
        
        // 468 랜드마크를 [x, y, z] 배열로 변환
        const landmarks = results.faceLandmarks[0].map(lm => [lm.x, lm.y, lm.z]);
        
        // Face Blendshapes (AU 대신 사용 가능)
        const blendshapes = results.faceBlendshapes?.[0]?.categories || [];
        
        return {
            landmarks: landmarks,
            blendshapes: blendshapes,
            confidence: 0.9,
        };
    } catch (e) {
        console.warn('Face detection error:', e);
        return null;
    }
};

// AU 계산 (Blendshapes 기반으로 변환)
window.computeAUs = function(landmarks) {
    if (!lastResults?.faceBlendshapes?.[0]) {
        // Blendshapes 없으면 기본값
        return {
            au1: 0, au2: 0, au4: 0, au5: 0, au6: 0,
            au12: 0, au15: 0, au25: 0, au26: 0
        };
    }
    
    const bs = {};
    lastResults.faceBlendshapes[0].categories.forEach(c => {
        bs[c.categoryName] = c.score;
    });
    
    // MediaPipe Blendshapes → FACS AU 매핑
    return {
        au1: (bs['browInnerUp'] || 0),                    // 눈썹 안쪽 올림
        au2: (bs['browOuterUpLeft'] + bs['browOuterUpRight']) / 2 || 0,  // 눈썹 바깥쪽 올림
        au4: (bs['browDownLeft'] + bs['browDownRight']) / 2 || 0,        // 눈썹 내림
        au5: (bs['eyeWideLeft'] + bs['eyeWideRight']) / 2 || 0,          // 눈 크게 뜸
        au6: (bs['cheekSquintLeft'] + bs['cheekSquintRight']) / 2 || 0,  // 볼 올림
        au12: (bs['mouthSmileLeft'] + bs['mouthSmileRight']) / 2 || 0,   // 입꼬리 올림
        au15: (bs['mouthFrownLeft'] + bs['mouthFrownRight']) / 2 || 0,   // 입꼬리 내림
        au25: (bs['mouthOpen'] || 0) * 0.5 + (bs['jawOpen'] || 0) * 0.5, // 입 벌림
        au26: bs['jawOpen'] || 0,                         // 턱 벌림
    };
};

// 감정 추론
window.inferEmotion = function(au) {
    if (!au) return null;
    
    const happiness = Math.min(1, au.au6 * 0.4 + au.au12 * 0.6);
    const sadness = Math.min(1, au.au1 * 0.3 + au.au4 * 0.3 + au.au15 * 0.4);
    const anger = Math.min(1, au.au4 * 0.5 + au.au5 * 0.2);
    const surprise = Math.min(1, au.au1 * 0.25 + au.au2 * 0.25 + au.au5 * 0.25 + au.au26 * 0.25);
    const disgust = Math.min(1, au.au15 * 0.5 + au.au4 * 0.3);
    
    const maxEmotion = Math.max(happiness, sadness, anger, surprise, disgust);
    const neutral = Math.max(0, 1 - maxEmotion * 1.5);
    
    // Dominant emotion
    const emotions = { happiness, sadness, anger, surprise, disgust, neutral };
    const dominant = Object.entries(emotions).reduce((a, b) => b[1] > a[1] ? b : a)[0];
    
    // VAD
    const valence = happiness - (sadness + anger + disgust) / 3;
    const arousal = (anger + surprise + happiness * 0.5) / 2.5;
    const dominance = 0.5 + anger * 0.3 - sadness * 0.3;
    
    return {
        happiness, sadness, anger, surprise, disgust, neutral,
        dominant,
        valence: Math.max(-1, Math.min(1, valence)),
        arousal: Math.max(0, Math.min(1, arousal)),
        dominance: Math.max(0, Math.min(1, dominance)),
    };
};

console.log('🎭 Face Analysis module loaded (Vision Tasks API)');
