//! Resemblyzer Voice Encoder - ONNX 기반 화자 임베딩
//! 
//! GE2E (Generalized End-to-End) LSTM 모델을 사용하여
//! 256차원 화자 임베딩을 생성합니다.

use std::collections::HashMap;
use std::sync::OnceLock;
use ndarray::{Array2, Array3, ArrayD, IxDyn};
use rustfft::{FftPlanner, num_complex::Complex};
use tract_onnx::prelude::*;

/// ONNX 모델 바이트 (컴파일 시 포함)
const MODEL_BYTES: &[u8] = include_bytes!("../../../models/resemblyzer/voice_encoder.onnx");

/// Mel spectrogram 설정 (resemblyzer 기본값)
const SAMPLE_RATE: f32 = 16000.0;
const N_FFT: usize = 400;       // 25ms at 16kHz
const HOP_LENGTH: usize = 160;  // 10ms at 16kHz  
const N_MELS: usize = 40;
const FMIN: f32 = 0.0;
const FMAX: f32 = 8000.0;

/// 고정 프레임 수 (ONNX 모델 입력 크기)
const N_FRAMES: usize = 160;

/// 화자 임베딩 (256차원)
pub type SpeakerEmbedding = [f32; 256];

/// Tract 모델 타입
type TractModel = SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

/// 전역 모델 (한 번만 로드)
static MODEL: OnceLock<Option<TractModel>> = OnceLock::new();

/// 모델 로드
fn get_model() -> Option<&'static TractModel> {
    MODEL.get_or_init(|| {
        load_model().ok()
    }).as_ref()
}

/// ONNX 모델 로드
fn load_model() -> TractResult<TractModel> {
    let model = tract_onnx::onnx()
        .model_for_read(&mut std::io::Cursor::new(MODEL_BYTES))?
        .with_input_fact(0, f32::fact([1, N_FRAMES, N_MELS]).into())?
        .into_optimized()?
        .into_runnable()?;
    
    Ok(model)
}

/// Resemblyzer 엔진
pub struct ResemblyzerEngine {
    /// 등록된 화자 임베딩
    speaker_embeddings: HashMap<String, Vec<SpeakerEmbedding>>,
    /// 유사도 임계값
    similarity_threshold: f32,
    /// Mel filterbank
    mel_filterbank: Array2<f32>,
    /// 모델 로드 여부
    model_available: bool,
}

impl ResemblyzerEngine {
    pub fn new() -> Self {
        // 모델 초기화 시도
        let model_available = get_model().is_some();
        
        if model_available {
            log::info!("✅ Resemblyzer ONNX 모델 로드 완료");
        } else {
            log::warn!("⚠️ ONNX 모델 로드 실패, pseudo-embedding 사용");
        }
        
        Self {
            speaker_embeddings: HashMap::new(),
            similarity_threshold: 0.75,
            mel_filterbank: create_mel_filterbank(N_FFT, N_MELS, SAMPLE_RATE, FMIN, FMAX),
            model_available,
        }
    }

    /// 오디오에서 Mel spectrogram 추출
    pub fn compute_mel_spectrogram(&self, audio: &[f32]) -> Array2<f32> {
        // 16kHz로 리샘플링이 필요하면 여기서 처리
        let n_frames = (audio.len().saturating_sub(N_FFT)) / HOP_LENGTH + 1;
        if n_frames == 0 {
            return Array2::zeros((0, N_MELS));
        }

        let mut mel_spec = Array2::zeros((n_frames, N_MELS));
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(N_FFT);

        for (frame_idx, start) in (0..audio.len().saturating_sub(N_FFT))
            .step_by(HOP_LENGTH)
            .enumerate()
        {
            if frame_idx >= n_frames {
                break;
            }

            // Hann 윈도우 적용
            let mut windowed: Vec<Complex<f32>> = audio[start..start + N_FFT]
                .iter()
                .enumerate()
                .map(|(i, &x)| {
                    let window = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (N_FFT - 1) as f32).cos();
                    Complex::new(x * window, 0.0)
                })
                .collect();

            // FFT
            fft.process(&mut windowed);

            // Power spectrum
            let power: Vec<f32> = windowed[..N_FFT / 2 + 1]
                .iter()
                .map(|c| c.norm_sqr())
                .collect();

            // Mel filterbank 적용
            for mel_idx in 0..N_MELS {
                let mut mel_energy = 0.0;
                for (freq_idx, &p) in power.iter().enumerate() {
                    if freq_idx < self.mel_filterbank.nrows() {
                        mel_energy += p * self.mel_filterbank[[freq_idx, mel_idx]];
                    }
                }
                // Log-mel
                mel_spec[[frame_idx, mel_idx]] = (mel_energy.max(1e-10)).ln();
            }
        }

        mel_spec
    }

    /// ONNX 모델로 화자 임베딩 추출
    pub fn extract_embedding(&self, mel_spec: &Array2<f32>) -> Option<SpeakerEmbedding> {
        if !self.model_available {
            return self.extract_embedding_fallback(mel_spec);
        }

        let model = get_model()?;

        // 프레임 수 조정 (N_FRAMES로 맞춤)
        let adjusted = self.adjust_frames(mel_spec);
        
        // 입력 텐서 생성: (1, N_FRAMES, N_MELS)
        let input: Tensor = tract_ndarray::Array3::from_shape_fn(
            (1, N_FRAMES, N_MELS),
            |(_, i, j)| adjusted[[i, j]]
        ).into();

        // 추론
        let result = model.run(tvec!(input.into())).ok()?;
        
        // 출력 파싱: (1, 256)
        let output = result[0].to_array_view::<f32>().ok()?;
        
        let mut embedding = [0.0f32; 256];
        for (i, &v) in output.iter().enumerate().take(256) {
            embedding[i] = v;
        }

        Some(embedding)
    }

    /// 프레임 수 조정 (padding/truncation)
    fn adjust_frames(&self, mel_spec: &Array2<f32>) -> Array2<f32> {
        let n_frames = mel_spec.nrows();
        
        if n_frames == N_FRAMES {
            return mel_spec.clone();
        }

        let mut adjusted = Array2::zeros((N_FRAMES, N_MELS));

        if n_frames > N_FRAMES {
            // 중앙 부분 추출
            let start = (n_frames - N_FRAMES) / 2;
            for i in 0..N_FRAMES {
                for j in 0..N_MELS {
                    adjusted[[i, j]] = mel_spec[[start + i, j]];
                }
            }
        } else {
            // 패딩 (양쪽)
            let pad_start = (N_FRAMES - n_frames) / 2;
            for i in 0..n_frames {
                for j in 0..N_MELS {
                    adjusted[[pad_start + i, j]] = mel_spec[[i, j]];
                }
            }
        }

        adjusted
    }

    /// Fallback: 통계 기반 pseudo-embedding
    fn extract_embedding_fallback(&self, mel_spec: &Array2<f32>) -> Option<SpeakerEmbedding> {
        if mel_spec.nrows() < 10 {
            return None;
        }

        let mut embedding = [0.0f32; 256];
        
        // Mel bands 평균/분산
        for mel_idx in 0..N_MELS {
            let col = mel_spec.column(mel_idx);
            let mean = col.mean().unwrap_or(0.0);
            let variance = col.mapv(|x| (x - mean).powi(2)).mean().unwrap_or(0.0);
            
            embedding[mel_idx] = mean;
            embedding[mel_idx + N_MELS] = variance.sqrt();
        }

        // Delta
        for mel_idx in 0..N_MELS {
            let mut delta_sum = 0.0;
            for i in 1..mel_spec.nrows() {
                delta_sum += (mel_spec[[i, mel_idx]] - mel_spec[[i - 1, mel_idx]]).abs();
            }
            embedding[mel_idx + 2 * N_MELS] = delta_sum / (mel_spec.nrows() - 1).max(1) as f32;
        }

        // L2 정규화
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            for x in &mut embedding {
                *x /= norm;
            }
        }

        Some(embedding)
    }

    /// 오디오에서 화자 식별
    pub fn identify_speaker(&mut self, audio: &[f32]) -> Option<String> {
        let mel_spec = self.compute_mel_spectrogram(audio);
        let embedding = self.extract_embedding(&mel_spec)?;

        // 기존 화자와 매칭
        let mut best_match = None;
        let mut best_similarity = self.similarity_threshold;

        for (speaker_id, embeddings) in &self.speaker_embeddings {
            for stored_emb in embeddings {
                let similarity = cosine_similarity(&embedding, stored_emb);
                if similarity > best_similarity {
                    best_similarity = similarity;
                    best_match = Some(speaker_id.clone());
                }
            }
        }

        match best_match {
            Some(id) => {
                self.update_speaker(&id, embedding);
                Some(id)
            }
            None => {
                let new_id = format!("Speaker_{}", self.speaker_embeddings.len() + 1);
                self.register_speaker(new_id.clone(), embedding);
                Some(new_id)
            }
        }
    }

    /// 새 화자 등록
    pub fn register_speaker(&mut self, speaker_id: String, embedding: SpeakerEmbedding) {
        self.speaker_embeddings
            .entry(speaker_id)
            .or_insert_with(Vec::new)
            .push(embedding);
    }

    /// 화자 프로필 업데이트
    pub fn update_speaker(&mut self, speaker_id: &str, embedding: SpeakerEmbedding) {
        if let Some(embeddings) = self.speaker_embeddings.get_mut(speaker_id) {
            embeddings.push(embedding);
            if embeddings.len() > 20 {
                embeddings.remove(0);
            }
        }
    }

    /// 등록된 화자 목록
    pub fn get_speakers(&self) -> Vec<String> {
        self.speaker_embeddings.keys().cloned().collect()
    }

    /// 두 임베딩 간 유사도
    pub fn compare_embeddings(&self, emb1: &SpeakerEmbedding, emb2: &SpeakerEmbedding) -> f32 {
        cosine_similarity(emb1, emb2)
    }

    /// 모델 사용 가능 여부
    pub fn is_model_available(&self) -> bool {
        self.model_available
    }

    /// 초기화
    pub fn reset(&mut self) {
        self.speaker_embeddings.clear();
    }
}

impl Default for ResemblyzerEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// 코사인 유사도
fn cosine_similarity(a: &[f32; 256], b: &[f32; 256]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a > 1e-6 && norm_b > 1e-6 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// Mel filterbank 생성
fn create_mel_filterbank(n_fft: usize, n_mels: usize, sample_rate: f32, fmin: f32, fmax: f32) -> Array2<f32> {
    let n_freqs = n_fft / 2 + 1;
    let mut filterbank = Array2::zeros((n_freqs, n_mels));

    let hz_to_mel = |hz: f32| 2595.0 * (1.0 + hz / 700.0).log10();
    let mel_to_hz = |mel: f32| 700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0);

    let mel_min = hz_to_mel(fmin);
    let mel_max = hz_to_mel(fmax);

    let mel_points: Vec<f32> = (0..=n_mels + 1)
        .map(|i| mel_min + (mel_max - mel_min) * i as f32 / (n_mels + 1) as f32)
        .collect();

    let hz_points: Vec<f32> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();

    let bin_points: Vec<usize> = hz_points
        .iter()
        .map(|&hz| ((n_fft as f32 + 1.0) * hz / sample_rate).floor() as usize)
        .collect();

    for m in 0..n_mels {
        let left = bin_points[m];
        let center = bin_points[m + 1];
        let right = bin_points[m + 2];

        for k in left..center {
            if k < n_freqs && center > left {
                filterbank[[k, m]] = (k - left) as f32 / (center - left) as f32;
            }
        }
        for k in center..right {
            if k < n_freqs && right > center {
                filterbank[[k, m]] = (right - k) as f32 / (right - center) as f32;
            }
        }
    }

    filterbank
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mel_filterbank() {
        let fb = create_mel_filterbank(400, 40, 16000.0, 0.0, 8000.0);
        assert_eq!(fb.shape(), &[201, 40]);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = [1.0f32; 256];
        let b = [1.0f32; 256];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-5);
    }
}
