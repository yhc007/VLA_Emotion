#!/bin/bash

# VLA-emotion 크래시 모니터링 스크립트
LOG_FILE="/tmp/vla_emotion_monitor.log"

echo "=== VLA-emotion 크래시 모니터링 시작 $(date) ===" >> $LOG_FILE

while true; do
    TIMESTAMP=$(date)
    
    # API Gateway 프로세스 확인
    API_PID=$(ps aux | grep -E "api-gateway" | grep -v grep | awk '{print $2}')
    TRUNK_PID=$(ps aux | grep -E "trunk serve" | grep -v grep | awk '{print $2}')
    
    if [[ -n "$API_PID" ]]; then
        # 메모리 사용량 확인
        API_MEM=$(ps -p $API_PID -o rss= | awk '{print $1}')
        echo "[$TIMESTAMP] API Gateway (PID: $API_PID) - Memory: ${API_MEM}KB" >> $LOG_FILE
    else
        echo "[$TIMESTAMP] ❌ API Gateway 크래시 감지!" >> $LOG_FILE
        echo "System info at crash:" >> $LOG_FILE
        vm_stat >> $LOG_FILE
        echo "---" >> $LOG_FILE
    fi
    
    if [[ -n "$TRUNK_PID" ]]; then
        TRUNK_MEM=$(ps -p $TRUNK_PID -o rss= | awk '{print $1}')
        echo "[$TIMESTAMP] Web UI (PID: $TRUNK_PID) - Memory: ${TRUNK_MEM}KB" >> $LOG_FILE
    else
        echo "[$TIMESTAMP] ❌ Web UI 크래시 감지!" >> $LOG_FILE
    fi
    
    # 시스템 메모리 압박 확인
    FREE_PAGES=$(vm_stat | grep "Pages free" | awk '{print $3}' | tr -d '.')
    if [[ $FREE_PAGES -lt 10000 ]]; then
        echo "[$TIMESTAMP] ⚠️  메모리 압박 감지! Free pages: $FREE_PAGES" >> $LOG_FILE
    fi
    
    sleep 30
done