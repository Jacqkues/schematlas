#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--mcp") {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        if runtime
            .block_on(schema_atlas_lib::agents::mcp::run())
            .is_err()
        {
            std::process::exit(1);
        }
    } else if std::env::args().nth(1).as_deref() == Some("--list-agents") {
        println!(
            "{}",
            serde_json::to_string_pretty(&schema_atlas_lib::agents::discovery::discover())
                .expect("agent inventory")
        );
    } else {
        schema_atlas_lib::run();
    }
}
