use leptos::prelude::*;

mod app;
mod api;
mod components;
mod types;
mod storage;
pub mod google;

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).expect("error initializing logger");
    
    // 데이터 저장소 초기화 — origin-relative.
    // server_url=""이면 storage 모듈이 `"/api/storage/..."` 형태로 호출, 같은 origin이면
    // dev/prod/SSH 터널 모두 자동 동작 (Trunk proxy / cloudflared ingress가 8080으로 포워딩).
    storage::initialize_storage("");
    log::info!("Data storage initialized (origin-relative)");
    
    leptos::mount::mount_to_body(app::App);
}
