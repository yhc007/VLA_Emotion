#!/usr/bin/env python3
"""
Resemblyzer Voice Encoder → ONNX 변환
GE2E (Generalized End-to-End) LSTM 모델
"""

import torch
import torch.nn as nn
import numpy as np
from pathlib import Path

# Resemblyzer의 VoiceEncoder 구조 재현
class VoiceEncoder(nn.Module):
    """
    GE2E Voice Encoder - 3-layer LSTM + Linear projection
    입력: (batch, n_frames, 40) mel spectrogram
    출력: (batch, 256) speaker embedding
    """
    def __init__(self):
        super().__init__()
        self.lstm = nn.LSTM(
            input_size=40,      # mel bins
            hidden_size=256,    # LSTM hidden
            num_layers=3,
            batch_first=True,
        )
        self.linear = nn.Linear(256, 256)
        self.relu = nn.ReLU()
        
    def forward(self, mels):
        # mels: (batch, n_frames, 40)
        _, (hidden, _) = self.lstm(mels)
        # hidden: (3, batch, 256) - take last layer
        embeds_raw = self.relu(self.linear(hidden[-1]))
        # L2 normalize
        embeds = embeds_raw / (torch.norm(embeds_raw, dim=1, keepdim=True) + 1e-5)
        return embeds


def load_pretrained_weights(model):
    """resemblyzer pretrained weights 로드"""
    from resemblyzer import VoiceEncoder as ResemblyzerEncoder
    
    # 원본 모델 로드
    original = ResemblyzerEncoder()
    
    # 가중치 복사
    with torch.no_grad():
        # LSTM weights
        model.lstm.weight_ih_l0.copy_(original.lstm.weight_ih_l0)
        model.lstm.weight_hh_l0.copy_(original.lstm.weight_hh_l0)
        model.lstm.bias_ih_l0.copy_(original.lstm.bias_ih_l0)
        model.lstm.bias_hh_l0.copy_(original.lstm.bias_hh_l0)
        
        model.lstm.weight_ih_l1.copy_(original.lstm.weight_ih_l1)
        model.lstm.weight_hh_l1.copy_(original.lstm.weight_hh_l1)
        model.lstm.bias_ih_l1.copy_(original.lstm.bias_ih_l1)
        model.lstm.bias_hh_l1.copy_(original.lstm.bias_hh_l1)
        
        model.lstm.weight_ih_l2.copy_(original.lstm.weight_ih_l2)
        model.lstm.weight_hh_l2.copy_(original.lstm.weight_hh_l2)
        model.lstm.bias_ih_l2.copy_(original.lstm.bias_ih_l2)
        model.lstm.bias_hh_l2.copy_(original.lstm.bias_hh_l2)
        
        # Linear weights
        model.linear.weight.copy_(original.linear.weight)
        model.linear.bias.copy_(original.linear.bias)
    
    return model


def export_onnx():
    print("🦊 Resemblyzer → ONNX 변환 시작")
    
    # 모델 생성 및 pretrained weights 로드
    model = VoiceEncoder()
    model = load_pretrained_weights(model)
    model.eval()
    
    # 더미 입력 (batch=1, frames=160, mel_bins=40)
    # 160 frames ≈ 1.6초 (10ms hop)
    dummy_input = torch.randn(1, 160, 40)
    
    # ONNX 내보내기
    output_path = Path(__file__).parent / "voice_encoder.onnx"
    
    # Export with fixed shape (LSTM requires static sequence length for some exporters)
    torch.onnx.export(
        model,
        dummy_input,
        str(output_path),
        export_params=True,
        opset_version=17,
        do_constant_folding=True,
        input_names=['mel_spectrogram'],
        output_names=['embedding'],
        # No dynamic_axes - use fixed shape for LSTM compatibility
        verbose=False,
        dynamo=False,  # Use legacy TorchScript-based export
    )
    
    print(f"✅ ONNX 저장: {output_path}")
    print(f"   파일 크기: {output_path.stat().st_size / 1024 / 1024:.2f} MB")
    
    # 검증
    import onnx
    onnx_model = onnx.load(output_path)
    onnx.checker.check_model(onnx_model)
    print("✅ ONNX 모델 검증 완료")
    
    # 원본과 비교 테스트
    print("\n🧪 정확도 테스트...")
    from resemblyzer import VoiceEncoder as OriginalEncoder
    original = OriginalEncoder()
    
    test_input = np.random.randn(1, 160, 40).astype(np.float32)
    
    # Original
    with torch.no_grad():
        original_out = original.forward(torch.from_numpy(test_input))
    
    # Our model
    with torch.no_grad():
        our_out = model(torch.from_numpy(test_input))
    
    diff = torch.abs(original_out - our_out).max().item()
    print(f"   최대 차이: {diff:.6f}")
    
    if diff < 1e-5:
        print("✅ 완벽 일치!")
    elif diff < 1e-3:
        print("✅ 거의 일치 (acceptable)")
    else:
        print("⚠️ 차이 있음 - 확인 필요")
    
    return output_path


if __name__ == "__main__":
    export_onnx()
