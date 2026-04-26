// STUB: Google Sheets / Drive / OAuth 연동 본 구현이 복원되기 전까지
// 컴파일만 통과시키는 최소 인터페이스.
//
// 정상 흐름에서 이 stub의 Err 경로는 실행되지 않는다 — `auth::GoogleLoginButton`이
// `on_login` 콜백을 발화하지 않으므로 `google_logged_in` 시그널이 false에 머무르고,
// app.rs의 Google 관련 Effect들은 모두 가드를 통과하지 못한다.
//
// 본 구현 복원 시 OAuth2 토큰 흐름, Sheets v4 API, Drive v3 API 호출이 필요.

pub struct GoogleSheets {
    _token: String,
}

impl GoogleSheets {
    pub fn new(token: &str) -> Self {
        Self {
            _token: token.to_string(),
        }
    }

    pub async fn create_session_spreadsheet(
        &self,
        _case_id: &str,
        _date: &str,
    ) -> Result<(String, String), String> {
        Err("Google Sheets stub: 본 구현 미복원".into())
    }

    pub async fn append_transcript(
        &self,
        _sheet_id: &str,
        _ts: &str,
        _speaker: &str,
        _text: &str,
    ) -> Result<(), String> {
        Err("Google Sheets stub".into())
    }

    pub async fn append_sentiment(
        &self,
        _sheet_id: &str,
        _ts: &str,
        _sentiment: &str,
        _confidence: f64,
        _extra: &str,
    ) -> Result<(), String> {
        Err("Google Sheets stub".into())
    }
}

pub struct GoogleDrive {
    _token: String,
}

impl GoogleDrive {
    pub fn new(token: &str) -> Self {
        Self {
            _token: token.to_string(),
        }
    }

    pub async fn get_or_create_vla_folder(&self) -> Result<String, String> {
        Err("Google Drive stub: 본 구현 미복원".into())
    }

    pub async fn move_to_folder(&self, _file_id: &str, _folder_id: &str) -> Result<(), String> {
        Err("Google Drive stub".into())
    }

    pub async fn upload_session_summary(
        &self,
        _folder_id: &str,
        _case_id: &str,
        _date: &str,
        _content: &str,
    ) -> Result<(String, String), String> {
        Err("Google Drive stub".into())
    }
}

pub mod auth {
    use leptos::prelude::*;

    #[component]
    pub fn GoogleLoginButton(
        client_id: String,
        on_login: Callback<String>,
        is_logged_in: ReadSignal<bool>,
    ) -> impl IntoView {
        // STUB: 실제 OAuth2 인증 없이 버튼만 표시. `on_login`은 발화하지 않는다.
        let _ = client_id;
        let _ = on_login;
        view! {
            <button
                class="text-[10px] text-grok-muted px-2 py-0.5 border border-grok-border rounded opacity-50 cursor-not-allowed"
                disabled=true
                title="Google 로그인 stub — 본 구현 복원 필요"
            >
                {move || if is_logged_in.get() { "Google: ON" } else { "Google: stub" }}
            </button>
        }
    }
}

pub mod drive {
    /// 세션 요약 마크다운 생성 — 본 구현은 더 풍부한 포맷 사용 예상.
    pub fn format_session_markdown(
        case_id: &str,
        date: &str,
        transcripts: &[(String, String, String)],
        sentiments: &[(String, String, f64)],
        _extra: Option<&str>,
    ) -> String {
        format!(
            "# Session {} ({})\n\n- transcripts: {}\n- sentiments: {}\n",
            case_id,
            date,
            transcripts.len(),
            sentiments.len()
        )
    }
}
