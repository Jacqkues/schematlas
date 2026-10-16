//! Pure schema geometry and contracts shared by the interface and layout worker.
pub mod diagram_layout;
#[path = "../../shared/explorer.rs"]
pub mod explorer;
pub mod flow_layout;
pub mod label;
pub mod layout;
pub mod openapi_layout;
pub mod relationships;
pub mod sequence_layout;
pub mod smart_layout;
#[path = "../../shared/toc.rs"]
pub mod toc;
pub mod types;
