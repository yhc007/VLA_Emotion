use leptos::prelude::*;

mod app;
mod api;
mod components;
mod types;

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).expect("error initializing logger");
    
    leptos::mount::mount_to_body(app::App);
}
