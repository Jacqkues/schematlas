//! The full model of every fixture, reviewed once and kept stable.
use schematlas_parser::mermaid;

#[test]
fn fixture_models() {
    insta::glob!("fixtures/mermaid/*.mmd", |path| {
        let src = std::fs::read_to_string(path).unwrap();
        insta::assert_yaml_snapshot!(mermaid::parse(&src).unwrap().diagram);
    });
}
