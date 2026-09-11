mod api;
mod components;
mod layout;
mod markdown;
mod preview;
mod relationships;
mod smart_layout;
mod state;
mod types;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(components::app::App);
}
