const http = require('http');
const PORT = 8080;

// 간단한 감정 분석 (규칙 기반)
function analyzeText(text) {
  const positiveWords = ['좋아', '감사', '행복', '기쁘', '최고', '훌륭', '멋지', '사랑', '고마워', '좋은', '잘', '성공', '완벽'];
  const negativeWords = ['싫어', '화나', '짜증', '슬프', '힘들', '어렵', '문제', '걱정', '불안', '나쁜', '못', '실패', '안되'];
  const questionWords = ['뭐', '어떻게', '왜', '언제', '어디', '누가', '할까', '인가', '일까', '?'];
  
  const lower = text.toLowerCase();
  
  let sentiment = 'neutral';
  let confidence = 0.5;
  let intent = 'statement';
  let keywords = [];
  
  // 감정 분석
  let posCount = 0, negCount = 0;
  for (const word of positiveWords) {
    if (lower.includes(word)) {
      posCount++;
      keywords.push(word);
    }
  }
  for (const word of negativeWords) {
    if (lower.includes(word)) {
      negCount++;
      keywords.push(word);
    }
  }
  
  if (posCount > negCount) {
    sentiment = 'positive';
    confidence = Math.min(0.5 + posCount * 0.15, 0.95);
  } else if (negCount > posCount) {
    sentiment = 'negative';
    confidence = Math.min(0.5 + negCount * 0.15, 0.95);
  }
  
  // 의도 분석
  for (const word of questionWords) {
    if (lower.includes(word)) {
      intent = 'question';
      break;
    }
  }
  if (lower.includes('해줘') || lower.includes('해 줘') || lower.includes('부탁')) {
    intent = 'request';
  }
  
  // 이모지 결정
  const emoji = sentiment === 'positive' ? '😊' : sentiment === 'negative' ? '😟' : '😐';
  
  return {
    sentiment,
    confidence,
    intent,
    keywords: keywords.slice(0, 3),
    emoji,
    summary: `${emoji} ${sentiment} (${Math.round(confidence * 100)}%) - ${intent}`
  };
}

const server = http.createServer((req, res) => {
  // CORS
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'POST, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
  
  if (req.method === 'OPTIONS') {
    res.writeHead(200);
    res.end();
    return;
  }

  // 샘 채팅 API
  if (req.method === 'POST' && req.url === '/api/chat') {
    let body = '';
    req.on('data', chunk => body += chunk);
    req.on('end', () => {
      try {
        const { message } = JSON.parse(body);
        console.log(`📨 채팅: ${message}`);
        
        const reply = `🦊 "${message}" 라고 했지? 잘 들렸어!`;
        
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ response: reply }));
      } catch (err) {
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ response: `오류: ${err.message}` }));
      }
    });
  }
  // 🧠 Neurosymbolic 추론 API
  else if (req.method === 'POST' && req.url === '/api/reason') {
    let body = '';
    req.on('data', chunk => body += chunk);
    req.on('end', async () => {
      try {
        const { text } = JSON.parse(body);
        console.log(`🧠 추론: ${text}`);
        
        // Python reasoner 호출
        const { execSync } = require('child_process');
        const result = execSync(
          `cd /Volumes/T7/Work/VLA_Emotion && source .venv/bin/activate && python neuro-symbolic/reasoner.py "${text.replace(/"/g, '\\"')}"`,
          { encoding: 'utf-8', shell: '/bin/zsh' }
        );
        
        // JSON 부분만 추출
        const jsonMatch = result.match(/\{[\s\S]*\}/);
        if (jsonMatch) {
          const parsed = JSON.parse(jsonMatch[0]);
          console.log(`   → ${parsed.summary?.split('\n')[0] || 'OK'}`);
          
          res.writeHead(200, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify(parsed));
        } else {
          res.writeHead(500, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ error: 'Parse error' }));
        }
      } catch (err) {
        console.error('추론 오류:', err.message);
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: err.message }));
      }
    });
  }
  // 🔍 문장 분석 API
  else if (req.method === 'POST' && req.url === '/api/analyze') {
    let body = '';
    req.on('data', chunk => body += chunk);
    req.on('end', () => {
      try {
        const { text, timestamp } = JSON.parse(body);
        console.log(`🔍 분석: ${text}`);
        
        const analysis = analyzeText(text);
        console.log(`   → ${analysis.summary}`);
        
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({
          text,
          timestamp: timestamp || Date.now(),
          ...analysis
        }));
      } catch (err) {
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: err.message }));
      }
    });
  }
  else {
    res.writeHead(404);
    res.end('Not Found');
  }
});

server.listen(PORT, () => {
  console.log(`🦊 VLA API Server running on http://localhost:${PORT}`);
  console.log(`📡 Endpoints:`);
  console.log(`   POST /api/chat    - 샘 채팅`);
  console.log(`   POST /api/analyze - 문장 분석`);
  console.log(`   POST /api/reason  - 🧠 Neurosymbolic 추론`);
});
