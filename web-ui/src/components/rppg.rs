//! rPPG (Remote Photoplethysmography) - 얼굴에서 심박수 추출
//! 
//! 원리: 피부 색상의 미세한 변화를 감지하여 혈류 변화 → 심박수 추정

use std::collections::VecDeque;
use std::f64::consts::PI;

/// rPPG 분석기 설정
pub const SAMPLE_RATE: f64 = 30.0;      // 카메라 FPS
pub const BUFFER_SECONDS: usize = 10;    // 분석에 사용할 초
pub const BUFFER_SIZE: usize = (SAMPLE_RATE as usize) * BUFFER_SECONDS;
pub const MIN_HR: f64 = 40.0;           // 최소 심박수 (BPM)
pub const MAX_HR: f64 = 180.0;          // 최대 심박수 (BPM)

/// 심박 분석 결과
#[derive(Clone, Debug, Default)]
pub struct HeartRateResult {
    pub bpm: f64,                    // 심박수 (beats per minute)
    pub confidence: f64,             // 신뢰도 (0~1)
    pub signal_quality: f64,         // 신호 품질 (0~1)
    pub hrv_sdnn: f64,              // 심박 변이도 (SDNN, ms)
    pub stress_index: f64,          // 스트레스 지수 (0~1)
    pub raw_signal: Vec<f64>,       // 원시 신호 (디버깅용)
    pub filtered_signal: Vec<f64>,  // 필터링된 신호
}

/// rPPG 분석기
pub struct RppgAnalyzer {
    /// Green 채널 버퍼
    green_buffer: VecDeque<f64>,
    /// 타임스탬프 버퍼
    timestamps: VecDeque<f64>,
    /// 이전 심박수 (스무딩용)
    prev_bpm: f64,
    /// IBI (Inter-Beat Interval) 버퍼 for HRV
    ibi_buffer: VecDeque<f64>,
}

impl Default for RppgAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl RppgAnalyzer {
    pub fn new() -> Self {
        Self {
            green_buffer: VecDeque::with_capacity(BUFFER_SIZE),
            timestamps: VecDeque::with_capacity(BUFFER_SIZE),
            prev_bpm: 70.0, // 초기값
            ibi_buffer: VecDeque::with_capacity(30),
        }
    }
    
    /// ROI에서 평균 Green 값 추출하여 버퍼에 추가
    pub fn add_sample(&mut self, green_mean: f64, timestamp: f64) {
        self.green_buffer.push_back(green_mean);
        self.timestamps.push_back(timestamp);
        
        // 버퍼 크기 유지
        while self.green_buffer.len() > BUFFER_SIZE {
            self.green_buffer.pop_front();
            self.timestamps.pop_front();
        }
    }
    
    /// RGB 픽셀 배열에서 Green 평균 계산
    pub fn extract_green_mean(pixels: &[u8]) -> f64 {
        if pixels.is_empty() {
            return 0.0;
        }
        
        let mut green_sum: u64 = 0;
        let mut count: u64 = 0;
        
        // RGBA 포맷 가정 (4바이트씩)
        for chunk in pixels.chunks(4) {
            if chunk.len() >= 3 {
                green_sum += chunk[1] as u64; // G 채널
                count += 1;
            }
        }
        
        if count > 0 {
            green_sum as f64 / count as f64
        } else {
            0.0
        }
    }
    
    /// 심박수 분석 실행
    pub fn analyze(&mut self) -> HeartRateResult {
        let mut result = HeartRateResult::default();
        
        if self.green_buffer.len() < BUFFER_SIZE / 2 {
            result.bpm = self.prev_bpm;
            result.confidence = 0.0;
            return result;
        }
        
        // 1. 신호 전처리
        let signal: Vec<f64> = self.green_buffer.iter().copied().collect();
        result.raw_signal = signal.clone();
        
        // 2. 트렌드 제거 (Detrending)
        let detrended = self.detrend(&signal);
        
        // 3. 밴드패스 필터 적용 (0.7~3Hz = 42~180 BPM)
        let filtered = self.bandpass_filter(&detrended, MIN_HR / 60.0, MAX_HR / 60.0);
        result.filtered_signal = filtered.clone();
        
        // 4. FFT로 주파수 분석
        let (bpm, confidence) = self.estimate_hr_fft(&filtered);
        
        // 5. 스무딩 (급격한 변화 방지)
        let smoothed_bpm = self.prev_bpm * 0.7 + bpm * 0.3;
        self.prev_bpm = smoothed_bpm;
        
        result.bpm = smoothed_bpm;
        result.confidence = confidence;
        result.signal_quality = self.calculate_signal_quality(&filtered);
        
        // 6. HRV 계산 (피크 검출 기반)
        if let Some(hrv) = self.calculate_hrv(&filtered) {
            result.hrv_sdnn = hrv;
            // 스트레스 지수: HRV가 낮을수록, HR이 높을수록 스트레스
            let hr_stress = ((smoothed_bpm - 60.0) / 60.0).clamp(0.0, 1.0);
            let hrv_stress = (1.0 - (hrv / 100.0)).clamp(0.0, 1.0);
            result.stress_index = hr_stress * 0.4 + hrv_stress * 0.6;
        }
        
        result
    }
    
    /// 선형 트렌드 제거
    fn detrend(&self, signal: &[f64]) -> Vec<f64> {
        let n = signal.len() as f64;
        let x_mean = (n - 1.0) / 2.0;
        let y_mean: f64 = signal.iter().sum::<f64>() / n;
        
        let mut num = 0.0;
        let mut den = 0.0;
        
        for (i, &y) in signal.iter().enumerate() {
            let x = i as f64;
            num += (x - x_mean) * (y - y_mean);
            den += (x - x_mean).powi(2);
        }
        
        let slope = if den != 0.0 { num / den } else { 0.0 };
        let intercept = y_mean - slope * x_mean;
        
        signal.iter().enumerate()
            .map(|(i, &y)| y - (slope * i as f64 + intercept))
            .collect()
    }
    
    /// 간단한 밴드패스 필터 (버터워스 근사)
    fn bandpass_filter(&self, signal: &[f64], low_hz: f64, high_hz: f64) -> Vec<f64> {
        let n = signal.len();
        if n < 4 {
            return signal.to_vec();
        }
        
        // 간단한 이동평균 기반 필터
        let low_window = (SAMPLE_RATE / high_hz).max(2.0) as usize;
        let high_window = (SAMPLE_RATE / low_hz).min(n as f64 / 2.0) as usize;
        
        // 고주파 통과 (트렌드 제거)
        let mut high_passed = vec![0.0; n];
        for i in high_window..n {
            let avg: f64 = signal[i-high_window..i].iter().sum::<f64>() / high_window as f64;
            high_passed[i] = signal[i] - avg;
        }
        
        // 저주파 통과 (노이즈 제거)
        let mut low_passed = vec![0.0; n];
        for i in low_window..n {
            low_passed[i] = high_passed[i-low_window..i].iter().sum::<f64>() / low_window as f64;
        }
        
        low_passed
    }
    
    /// FFT 기반 심박수 추정
    fn estimate_hr_fft(&self, signal: &[f64]) -> (f64, f64) {
        let n = signal.len();
        if n < 64 {
            return (self.prev_bpm, 0.0);
        }
        
        // 간단한 DFT (작은 범위만 계산)
        let min_freq = MIN_HR / 60.0;
        let max_freq = MAX_HR / 60.0;
        let freq_resolution = SAMPLE_RATE / n as f64;
        
        let min_bin = (min_freq / freq_resolution) as usize;
        let max_bin = ((max_freq / freq_resolution) as usize).min(n / 2);
        
        let mut max_power = 0.0;
        let mut max_freq_hz = 1.0;
        let mut total_power = 0.0;
        
        for k in min_bin..max_bin {
            let freq = k as f64 * freq_resolution;
            let mut real = 0.0;
            let mut imag = 0.0;
            
            for (i, &x) in signal.iter().enumerate() {
                let angle = 2.0 * PI * k as f64 * i as f64 / n as f64;
                real += x * angle.cos();
                imag -= x * angle.sin();
            }
            
            let power = real * real + imag * imag;
            total_power += power;
            
            if power > max_power {
                max_power = power;
                max_freq_hz = freq;
            }
        }
        
        let bpm = max_freq_hz * 60.0;
        let confidence = if total_power > 0.0 {
            (max_power / total_power).sqrt().min(1.0)
        } else {
            0.0
        };
        
        (bpm.clamp(MIN_HR, MAX_HR), confidence)
    }
    
    /// 신호 품질 계산
    fn calculate_signal_quality(&self, signal: &[f64]) -> f64 {
        if signal.is_empty() {
            return 0.0;
        }
        
        let mean: f64 = signal.iter().sum::<f64>() / signal.len() as f64;
        let variance: f64 = signal.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / signal.len() as f64;
        let std_dev = variance.sqrt();
        
        // SNR 근사 (신호 대 잡음비)
        let max_amplitude = signal.iter()
            .map(|x| (x - mean).abs())
            .fold(0.0, f64::max);
        
        if std_dev > 0.0 {
            (max_amplitude / (std_dev * 3.0)).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
    
    /// HRV (SDNN) 계산 - 피크 간격의 표준편차
    fn calculate_hrv(&mut self, signal: &[f64]) -> Option<f64> {
        // 피크 검출
        let peaks = self.detect_peaks(signal);
        
        if peaks.len() < 3 {
            return None;
        }
        
        // IBI (Inter-Beat Interval) 계산
        let mut ibis: Vec<f64> = Vec::new();
        for i in 1..peaks.len() {
            let ibi_samples = peaks[i] - peaks[i-1];
            let ibi_ms = (ibi_samples as f64 / SAMPLE_RATE) * 1000.0;
            
            // 유효한 IBI만 포함 (300ms ~ 1500ms = 40~200 BPM)
            if (300.0..=1500.0).contains(&ibi_ms) {
                ibis.push(ibi_ms);
                self.ibi_buffer.push_back(ibi_ms);
            }
        }
        
        // IBI 버퍼 크기 제한
        while self.ibi_buffer.len() > 30 {
            self.ibi_buffer.pop_front();
        }
        
        if self.ibi_buffer.len() < 5 {
            return None;
        }
        
        // SDNN 계산 (IBI의 표준편차)
        let ibi_vec: Vec<f64> = self.ibi_buffer.iter().copied().collect();
        let mean: f64 = ibi_vec.iter().sum::<f64>() / ibi_vec.len() as f64;
        let variance: f64 = ibi_vec.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / ibi_vec.len() as f64;
        
        Some(variance.sqrt())
    }
    
    /// 간단한 피크 검출
    fn detect_peaks(&self, signal: &[f64]) -> Vec<usize> {
        let mut peaks = Vec::new();
        let n = signal.len();
        
        if n < 3 {
            return peaks;
        }
        
        // 적응형 임계값 계산
        let mean: f64 = signal.iter().sum::<f64>() / n as f64;
        let std_dev: f64 = (signal.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / n as f64).sqrt();
        let threshold = mean + std_dev * 0.5;
        
        // 최소 피크 간격 (40 BPM 기준)
        let min_distance = (SAMPLE_RATE * 60.0 / MAX_HR) as usize;
        
        let mut last_peak = 0;
        
        for i in 1..n-1 {
            if signal[i] > signal[i-1] 
                && signal[i] > signal[i+1] 
                && signal[i] > threshold
                && (i - last_peak) >= min_distance
            {
                peaks.push(i);
                last_peak = i;
            }
        }
        
        peaks
    }
    
    /// 버퍼 초기화
    pub fn reset(&mut self) {
        self.green_buffer.clear();
        self.timestamps.clear();
        self.ibi_buffer.clear();
        self.prev_bpm = 70.0;
    }
}

/// 긴장 상태 판단
#[derive(Clone, Debug, PartialEq)]
pub enum TensionLevel {
    Relaxed,    // 이완 (HR < 70, HRV > 50)
    Normal,     // 보통
    Mild,       // 약간 긴장 (HR 80-100)
    Moderate,   // 중간 긴장 (HR 100-120)
    High,       // 높은 긴장 (HR > 120 or HRV < 20)
}

impl TensionLevel {
    pub fn from_result(result: &HeartRateResult) -> Self {
        if result.confidence < 0.3 {
            return TensionLevel::Normal; // 신뢰도 낮으면 기본값
        }
        
        match (result.bpm, result.hrv_sdnn, result.stress_index) {
            (hr, hrv, _) if hr < 70.0 && hrv > 50.0 => TensionLevel::Relaxed,
            (hr, _, stress) if hr > 120.0 || stress > 0.8 => TensionLevel::High,
            (hr, hrv, _) if hr > 100.0 || hrv < 30.0 => TensionLevel::Moderate,
            (hr, _, _) if hr > 80.0 => TensionLevel::Mild,
            _ => TensionLevel::Normal,
        }
    }
    
    pub fn emoji(&self) -> &'static str {
        match self {
            TensionLevel::Relaxed => "😌",
            TensionLevel::Normal => "😐",
            TensionLevel::Mild => "😬",
            TensionLevel::Moderate => "😰",
            TensionLevel::High => "😱",
        }
    }
    
    pub fn label(&self) -> &'static str {
        match self {
            TensionLevel::Relaxed => "이완",
            TensionLevel::Normal => "보통",
            TensionLevel::Mild => "약간 긴장",
            TensionLevel::Moderate => "긴장",
            TensionLevel::High => "매우 긴장",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_analyzer_creation() {
        let analyzer = RppgAnalyzer::new();
        assert_eq!(analyzer.green_buffer.len(), 0);
    }
    
    #[test]
    fn test_add_sample() {
        let mut analyzer = RppgAnalyzer::new();
        for i in 0..100 {
            analyzer.add_sample(128.0 + (i as f64 * 0.1).sin() * 10.0, i as f64 / 30.0);
        }
        assert_eq!(analyzer.green_buffer.len(), 100);
    }
    
    #[test]
    fn test_green_extraction() {
        // RGBA: [R, G, B, A]
        let pixels = vec![100u8, 150, 120, 255, 110, 160, 130, 255];
        let green_mean = RppgAnalyzer::extract_green_mean(&pixels);
        assert!((green_mean - 155.0).abs() < 1.0);
    }
    
    #[test]
    fn test_tension_level() {
        let mut result = HeartRateResult::default();
        result.bpm = 65.0;
        result.hrv_sdnn = 60.0;
        result.confidence = 0.8;
        
        assert_eq!(TensionLevel::from_result(&result), TensionLevel::Relaxed);
        
        result.bpm = 125.0;
        assert_eq!(TensionLevel::from_result(&result), TensionLevel::High);
    }
}
