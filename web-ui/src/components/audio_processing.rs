use ndarray::{Array1, Array2};
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;
use std::collections::HashMap;

/// 화자 특징 벡터
#[derive(Clone, Debug)]
pub struct VoiceFeatures {
    pub f0: Option<f32>,           // 기본 주파수
    pub spectral_centroid: f32,    // 스펙트럴 센트로이드  
    pub mfcc: Vec<f32>,            // MFCC 특징 (처음 13개)
    pub energy: f32,               // 에너지
    pub zcr: f32,                  // Zero Crossing Rate
}

/// 화자 프로필
#[derive(Clone, Debug)]
pub struct SpeakerProfile {
    pub id: String,
    pub features: Vec<VoiceFeatures>,
    pub centroid: VoiceFeatures,
    pub confidence: f32,
}

/// 화자 분리 엔진
pub struct SpeakerDiarizer {
    speaker_profiles: HashMap<String, SpeakerProfile>,
    similarity_threshold: f32,
    sample_rate: f32,
    frame_size: usize,
    hop_size: usize,
}

impl SpeakerDiarizer {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            speaker_profiles: HashMap::new(),
            similarity_threshold: 0.7,
            sample_rate,
            frame_size: 1024,
            hop_size: 512,
        }
    }

    /// 오디오 청크를 분석하여 화자 식별
    pub fn process_audio_chunk(&mut self, audio_data: &[f32]) -> Option<String> {
        if audio_data.len() < self.frame_size {
            return None;
        }

        // 음성 특징 추출
        let features = self.extract_features(audio_data);

        // 기존 화자들과 매칭 시도
        if let Some(speaker_id) = self.find_best_match(&features) {
            self.update_speaker_profile(&speaker_id, features);
            Some(speaker_id)
        } else {
            // 새로운 화자 등록
            let new_speaker_id = format!("Speaker_{}", self.speaker_profiles.len() + 1);
            self.create_new_speaker_profile(new_speaker_id.clone(), features);
            Some(new_speaker_id)
        }
    }

    /// 음성 특징 추출
    fn extract_features(&self, audio_data: &[f32]) -> VoiceFeatures {
        let energy = self.calculate_energy(audio_data);
        let zcr = self.calculate_zero_crossing_rate(audio_data);
        let f0 = self.estimate_fundamental_frequency(audio_data);
        let spectral_centroid = self.calculate_spectral_centroid(audio_data);
        let mfcc = self.extract_mfcc(audio_data);

        VoiceFeatures {
            f0,
            spectral_centroid,
            mfcc,
            energy,
            zcr,
        }
    }

    /// 에너지 계산
    fn calculate_energy(&self, audio: &[f32]) -> f32 {
        audio.iter().map(|x| x * x).sum::<f32>() / audio.len() as f32
    }

    /// Zero Crossing Rate 계산
    fn calculate_zero_crossing_rate(&self, audio: &[f32]) -> f32 {
        let mut zcr = 0;
        for i in 1..audio.len() {
            if (audio[i] >= 0.0) != (audio[i - 1] >= 0.0) {
                zcr += 1;
            }
        }
        zcr as f32 / audio.len() as f32
    }

    /// 기본 주파수 추정 (간단한 autocorrelation 기반)
    fn estimate_fundamental_frequency(&self, audio: &[f32]) -> Option<f32> {
        let min_period = (self.sample_rate / 500.0) as usize; // 최대 500Hz
        let max_period = (self.sample_rate / 80.0) as usize;  // 최소 80Hz
        
        if audio.len() < max_period * 2 {
            return None;
        }

        let mut best_correlation = 0.0;
        let mut best_period = 0;

        for period in min_period..max_period {
            let mut correlation = 0.0;
            let mut norm1 = 0.0;
            let mut norm2 = 0.0;

            for i in 0..(audio.len() - period) {
                correlation += audio[i] * audio[i + period];
                norm1 += audio[i] * audio[i];
                norm2 += audio[i + period] * audio[i + period];
            }

            if norm1 > 0.0 && norm2 > 0.0 {
                correlation /= (norm1 * norm2).sqrt();
                if correlation > best_correlation {
                    best_correlation = correlation;
                    best_period = period;
                }
            }
        }

        if best_correlation > 0.3 {
            Some(self.sample_rate / best_period as f32)
        } else {
            None
        }
    }

    /// 스펙트럴 센트로이드 계산
    fn calculate_spectral_centroid(&self, audio: &[f32]) -> f32 {
        let fft_size = 512;
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        // 윈도우 적용
        let windowed: Vec<_> = audio
            .iter()
            .take(fft_size)
            .enumerate()
            .map(|(i, &x)| {
                let window = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos();
                rustfft::num_complex::Complex::new(x * window, 0.0)
            })
            .collect();

        let mut spectrum = windowed;
        fft.process(&mut spectrum);

        // 스펙트럴 센트로이드 계산
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for (i, bin) in spectrum.iter().enumerate().take(fft_size / 2) {
            let magnitude = bin.norm();
            let frequency = i as f32 * self.sample_rate / fft_size as f32;
            numerator += frequency * magnitude;
            denominator += magnitude;
        }

        if denominator > 0.0 {
            numerator / denominator
        } else {
            0.0
        }
    }

    /// 간단한 MFCC 특징 추출 (DCT 없이 Mel filterbank만)
    fn extract_mfcc(&self, audio: &[f32]) -> Vec<f32> {
        let fft_size = 512;
        let mel_filters = 13;
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);

        // 윈도우 적용 및 FFT
        let windowed: Vec<_> = audio
            .iter()
            .take(fft_size)
            .enumerate()
            .map(|(i, &x)| {
                let window = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos();
                rustfft::num_complex::Complex::new(x * window, 0.0)
            })
            .collect();

        let mut spectrum = windowed;
        fft.process(&mut spectrum);

        // 파워 스펙트럼
        let power_spectrum: Vec<f32> = spectrum
            .iter()
            .take(fft_size / 2)
            .map(|c| c.norm_sqr())
            .collect();

        // 간단한 Mel filterbank (실제로는 더 복잡함)
        let mut mel_features = Vec::with_capacity(mel_filters);
        let freq_step = power_spectrum.len() / mel_filters;

        for i in 0..mel_filters {
            let start_idx = i * freq_step;
            let end_idx = ((i + 1) * freq_step).min(power_spectrum.len());
            let mel_energy: f32 = power_spectrum[start_idx..end_idx].iter().sum();
            mel_features.push(mel_energy.ln().max(-50.0)); // log with floor
        }

        mel_features
    }

    /// 기존 화자와 매칭 시도
    fn find_best_match(&self, features: &VoiceFeatures) -> Option<String> {
        let mut best_similarity = 0.0;
        let mut best_speaker = None;

        for (speaker_id, profile) in &self.speaker_profiles {
            let similarity = self.calculate_similarity(features, &profile.centroid);
            if similarity > best_similarity && similarity > self.similarity_threshold {
                best_similarity = similarity;
                best_speaker = Some(speaker_id.clone());
            }
        }

        best_speaker
    }

    /// 특징 벡터 간 유사도 계산
    fn calculate_similarity(&self, features1: &VoiceFeatures, features2: &VoiceFeatures) -> f32 {
        let mut similarity_sum = 0.0;
        let mut weight_sum = 0.0;

        // F0 유사도 (30% 가중치)
        if let (Some(f1), Some(f2)) = (features1.f0, features2.f0) {
            let f0_similarity = 1.0 - (f1 - f2).abs() / (f1 + f2).max(1.0);
            similarity_sum += f0_similarity * 0.3;
            weight_sum += 0.3;
        }

        // 스펙트럴 센트로이드 유사도 (20% 가중치)
        let centroid_diff = (features1.spectral_centroid - features2.spectral_centroid).abs();
        let centroid_similarity = 1.0 - centroid_diff / (features1.spectral_centroid + features2.spectral_centroid).max(1.0);
        similarity_sum += centroid_similarity * 0.2;
        weight_sum += 0.2;

        // MFCC 코사인 유사도 (40% 가중치)
        let mfcc_similarity = self.cosine_similarity(&features1.mfcc, &features2.mfcc);
        similarity_sum += mfcc_similarity * 0.4;
        weight_sum += 0.4;

        // 에너지 유사도 (10% 가중치)
        let energy_similarity = 1.0 - (features1.energy - features2.energy).abs() / (features1.energy + features2.energy).max(0.01);
        similarity_sum += energy_similarity * 0.1;
        weight_sum += 0.1;

        if weight_sum > 0.0 {
            similarity_sum / weight_sum
        } else {
            0.0
        }
    }

    /// 코사인 유사도 계산
    fn cosine_similarity(&self, vec1: &[f32], vec2: &[f32]) -> f32 {
        let len = vec1.len().min(vec2.len());
        if len == 0 {
            return 0.0;
        }

        let mut dot_product = 0.0;
        let mut norm1 = 0.0;
        let mut norm2 = 0.0;

        for i in 0..len {
            dot_product += vec1[i] * vec2[i];
            norm1 += vec1[i] * vec1[i];
            norm2 += vec2[i] * vec2[i];
        }

        if norm1 > 0.0 && norm2 > 0.0 {
            dot_product / (norm1.sqrt() * norm2.sqrt())
        } else {
            0.0
        }
    }

    /// 새로운 화자 프로필 생성
    fn create_new_speaker_profile(&mut self, speaker_id: String, features: VoiceFeatures) {
        let profile = SpeakerProfile {
            id: speaker_id.clone(),
            centroid: features.clone(),
            features: vec![features],
            confidence: 1.0,
        };
        self.speaker_profiles.insert(speaker_id, profile);
    }

    /// 기존 화자 프로필 업데이트
    fn update_speaker_profile(&mut self, speaker_id: &str, features: VoiceFeatures) {
        if let Some(profile) = self.speaker_profiles.get_mut(speaker_id) {
            profile.features.push(features.clone());
            
            // 최근 N개 특징만 유지
            if profile.features.len() > 10 {
                profile.features.remove(0);
            }

            // 센트로이드 업데이트 (이동 평균)
            let features_copy = profile.features.clone();
            profile.centroid = Self::calculate_centroid(&features_copy);
        }
    }

    /// 특징 벡터들의 센트로이드 계산
    fn calculate_centroid(features_list: &[VoiceFeatures]) -> VoiceFeatures {
        if features_list.is_empty() {
            return VoiceFeatures {
                f0: None,
                spectral_centroid: 0.0,
                mfcc: vec![],
                energy: 0.0,
                zcr: 0.0,
            };
        }

        let count = features_list.len() as f32;
        let mut avg_f0_sum = 0.0;
        let mut avg_f0_count = 0;
        let mut avg_centroid = 0.0;
        let mut avg_energy = 0.0;
        let mut avg_zcr = 0.0;

        let mfcc_len = features_list.first().map(|f| f.mfcc.len()).unwrap_or(0);
        let mut avg_mfcc = vec![0.0; mfcc_len];

        for features in features_list {
            if let Some(f0) = features.f0 {
                avg_f0_sum += f0;
                avg_f0_count += 1;
            }
            avg_centroid += features.spectral_centroid;
            avg_energy += features.energy;
            avg_zcr += features.zcr;

            for (i, &mfcc_val) in features.mfcc.iter().enumerate() {
                if i < avg_mfcc.len() {
                    avg_mfcc[i] += mfcc_val;
                }
            }
        }

        VoiceFeatures {
            f0: if avg_f0_count > 0 { Some(avg_f0_sum / avg_f0_count as f32) } else { None },
            spectral_centroid: avg_centroid / count,
            mfcc: avg_mfcc.iter().map(|&x| x / count).collect(),
            energy: avg_energy / count,
            zcr: avg_zcr / count,
        }
    }

    /// 등록된 화자 목록 반환
    pub fn get_speakers(&self) -> Vec<String> {
        self.speaker_profiles.keys().cloned().collect()
    }

    /// 화자 프로필 정보 반환
    pub fn get_speaker_info(&self, speaker_id: &str) -> Option<&SpeakerProfile> {
        self.speaker_profiles.get(speaker_id)
    }
}