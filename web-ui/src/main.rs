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
    
    // 데이터 저장소 초기화 (API 서버 URL)
    storage::initialize_storage("http://localhost:8080");
    log::info!("Data storage initialized with server: http://localhost:8080");
    
    leptos::mount::mount_to_body(app::App);
}
