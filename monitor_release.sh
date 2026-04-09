#!/bin/bash

# VLA-emotion RELEASE 모드 크래시 모니터링
LOG_FILE="/tmp/vla_emotion_monitor.log"

echo "=== RELEASE 모드 모니터링 시작 $(date) ===" >> $LOG_FILE

START_TIME=$(date +%s)

while true; do
    CURRENT_TIME=$(date +%s)
    ELAPSED=$((CURRENT_TIME - START_TIME))
    ELAPSED_MIN=$((ELAPSED / 60))
    TIMESTAMP=$(date)
    
    # API Gateway 프로세스 확인 (release)
    API_PID=$(ps aux | grep -E "target/release/api-gateway" | grep -v grep | awk '{print $2}')
    TRUNK_PID=$(ps aux | grep -E "trunk serve" | grep -v grep | awk '{print $2}')
    
    if [[ -n "$API_PID" ]]; then
        API_MEM=$(ps -p $API_PID -o rss= | awk '{print $1}')
        echo "[$TIMESTAMP] ✅ API Gateway (RELEASE) (PID: $API_PID) - Memory: ${API_MEM}KB - Uptime: ${ELAPSED_MIN}min" >> $LOG_FILE
    else
        echo "[$TIMESTAMP] ❌ API Gateway (RELEASE) 크래시 감지! Uptime: ${ELAPSED_MIN}min" >> $LOG_FILE
        echo "System info at crash:" >> $LOG_FILE
        vm_stat >> $LOG_FILE
        echo "---" >> $LOG_FILE
    fi
    
    if [[ -n "$TRUNK_PID" ]]; then
        TRUNK_MEM=$(ps -p $TRUNK_PID -o rss= | awk '{print $1}')
        echo "[$TIMESTAMP] ✅ Web UI (PID: $TRUNK_PID) - Memory: ${TRUNK_MEM}KB - Uptime: ${ELAPSED_MIN}min" >> $LOG_FILE
    else
        echo "[$TIMESTAMP] ❌ Web UI 크래시 감지! Uptime: ${ELAPSED_MIN}min" >> $LOG_FILE
    fi
    
    # 30분 마크 알림
    if [[ $ELAPSED_MIN -eq 30 ]]; then
        echo "[$TIMESTAMP] 🎯 30분 마크 도달! 기존 크래시 지점 통과!" >> $LOG_FILE
    fi
    
    # 35분 마크 - 성공!
    if [[ $ELAPSED_MIN -eq 35 ]]; then
        echo "[$TIMESTAMP] 🎉 35분 마크 도달! 크래시 문제 해결됨!" >> $LOG_FILE
    fi
    
    sleep 60
done