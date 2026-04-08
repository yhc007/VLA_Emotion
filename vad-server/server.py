#!/usr/bin/env python3
"""
Silero VAD WebSocket Server
실시간 음성 종료 감지
"""

import asyncio
import json
import torch
import numpy as np
from silero_vad import load_silero_vad, get_speech_timestamps
import websockets

# Silero VAD 모델 로드
print("🎤 Loading Silero VAD model...")
model = load_silero_vad()
print("✅ Silero VAD ready!")

# 설정
SAMPLE_RATE = 16000
CHUNK_SIZE = 512  # 32ms at 16kHz
SPEECH_THRESHOLD = 0.5
SILENCE_DURATION_MS = 800  # 0.8초 침묵 후 종료

async def handle_client(websocket):
    """WebSocket 클라이언트 처리"""
    print(f"🔗 Client connected")
    
    audio_buffer = []
    is_speaking = False
    silence_start = None
    
    try:
        async for message in websocket:
            if isinstance(message, bytes):
                # Float32 PCM 오디오 데이터
                audio_chunk = np.frombuffer(message, dtype=np.float32)
                audio_buffer.extend(audio_chunk)
                
                # 충분한 데이터가 모이면 VAD 실행
                if len(audio_buffer) >= CHUNK_SIZE:
                    chunk = torch.tensor(audio_buffer[:CHUNK_SIZE])
                    audio_buffer = audio_buffer[CHUNK_SIZE:]
                    
                    # VAD 예측
                    speech_prob = model(chunk, SAMPLE_RATE).item()
                    
                    # 상태 업데이트
                    if speech_prob > SPEECH_THRESHOLD:
                        if not is_speaking:
                            is_speaking = True
                            await websocket.send(json.dumps({
                                "event": "speech_start",
                                "probability": speech_prob
                            }))
                            print(f"🗣️ Speech started (prob: {speech_prob:.2f})")
                        silence_start = None
                    else:
                        if is_speaking:
                            if silence_start is None:
                                silence_start = asyncio.get_event_loop().time()
                            else:
                                silence_duration = (asyncio.get_event_loop().time() - silence_start) * 1000
                                if silence_duration >= SILENCE_DURATION_MS:
                                    is_speaking = False
                                    silence_start = None
                                    await websocket.send(json.dumps({
                                        "event": "speech_end",
                                        "silence_ms": silence_duration
                                    }))
                                    print(f"🔇 Speech ended (silence: {silence_duration:.0f}ms)")
                    
                    # 실시간 확률 전송 (디버깅용)
                    await websocket.send(json.dumps({
                        "event": "vad_probability",
                        "probability": speech_prob,
                        "is_speaking": is_speaking
                    }))
                    
            elif isinstance(message, str):
                # JSON 명령
                cmd = json.loads(message)
                if cmd.get("action") == "reset":
                    audio_buffer = []
                    is_speaking = False
                    silence_start = None
                    print("🔄 Reset")
                    
    except websockets.exceptions.ConnectionClosed:
        print(f"🔌 Client disconnected")

async def main():
    """서버 시작"""
    port = 8085
    print(f"🚀 Starting Silero VAD WebSocket Server on ws://localhost:{port}")
    
    async with websockets.serve(handle_client, "localhost", port):
        await asyncio.Future()  # 무한 대기

if __name__ == "__main__":
    asyncio.run(main())
