mod api;
mod appearance;
mod components;
use schematlas_ui::layout;
mod markdown;
mod preview;
use schematlas_ui::relationships;
mod layout_worker;
mod state;
use schematlas_ui::types;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(components::app::App);
}
