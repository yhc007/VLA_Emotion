//! Application State

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use pekko_agent_events::publisher::EventPublisher;
use crate::actors::VlaPsychAgent;

/// 공유 애플리케이션 상태
#[derive(Clone)]
pub struct AppState {
    /// 활성 세션 (session_id → agent)
    pub sessions: Arc<RwLock<HashMap<Uuid, VlaPsychAgent>>>,
    /// 이벤트 퍼블리셔
    pub event_publisher: Arc<EventPublisher>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            event_publisher: Arc::new(EventPublisher::new("vla", 256)),
        }
    }

    /// 새 세션 생성
    pub async fn create_session(&self) -> Uuid {
        let session_id = Uuid::new_v4();
        let agent_id = format!("vla-psych-{}", &session_id.to_string()[..8]);
        let agent = VlaPsychAgent::new(&agent_id);
        
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, agent);
        
        session_id
    }

    /// 세션 가져오기
    pub async fn get_session(&self, session_id: &Uuid) -> Option<VlaPsychAgent> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// 세션 업데이트
    pub async fn update_session(&self, session_id: &Uuid, agent: VlaPsychAgent) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(*session_id, agent);
    }

    /// 세션 존재 여부
    pub async fn session_exists(&self, session_id: &Uuid) -> bool {
        let sessions = self.sessions.read().await;
        sessions.contains_key(session_id)
    }

    /// 활성 세션 수
    pub async fn session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
