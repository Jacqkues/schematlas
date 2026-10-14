//! Mermaid blocks inside Markdown files.
use schematlas_parser::markdown;
use schematlas_parser::mermaid;
use schematlas_parser::model::Edit;

const DOC: &str = "# Architecture\n\nSome text.\n\n```mermaid\nflowchart LR\n  a --> b\n```\n\n## Deploy\n\n~~~mermaid\ngraph TD\n  x --> y\n~~~\n\n```rust\nfn main() {}\n```\n\n- list item\n\n  ```mermaid\n  flowchart LR\n    nested\n  ```\n\n````markdown\n```mermaid\nflowchart LR\n  not-a-block\n```\n````\n";

#[test]
fn finds_every_mermaid_fence_with_its_byte_range_and_heading() {
    let blocks = markdown::mermaid_blocks(DOC);
    assert_eq!(blocks.len(), 3, "{blocks:#?}");
    assert_eq!(&DOC[blocks[0].range.clone()], "flowchart LR\n  a --> b\n");
    assert_eq!(blocks[0].heading.as_deref(), Some("Architecture"));
    assert_eq!(&DOC[blocks[1].range.clone()], "graph TD\n  x --> y\n");
    assert_eq!(blocks[1].heading.as_deref(), Some("Deploy"));
    // Indented fences inside lists keep their indentation stripped for parsing.
    assert_eq!(blocks[2].source(DOC), "flowchart LR\n  nested\n");
    assert_eq!(
        blocks.iter().map(|b| b.index).collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn replacing_a_block_leaves_the_rest_of_the_document_byte_identical() {
    let blocks = markdown::mermaid_blocks(DOC);
    let updated = markdown::replace_block(DOC, &blocks[1], "graph TD\n  x --> y\n  y --> z\n");
    assert!(updated.contains("  y --> z\n~~~"));
    let before = &DOC[..blocks[1].range.start];
    assert!(updated.starts_with(before));
    assert!(updated.ends_with(&DOC[blocks[1].range.end..]));
    // Indented blocks are re-indented on the way back.
    let updated = markdown::replace_block(DOC, &blocks[2], "flowchart LR\n  nested --> more\n");
    assert!(
        updated.contains("  ```mermaid\n  flowchart LR\n    nested --> more\n  ```"),
        "{updated}"
    );
}

#[test]
fn unterminated_fences_and_empty_documents() {
    assert!(markdown::mermaid_blocks("").is_empty());
    assert!(markdown::mermaid_blocks("```mermaid\nflowchart LR\n  a").len() <= 1);
    assert!(markdown::mermaid_blocks("text ```mermaid inline``` text").is_empty());
}

#[test]
fn diagrams_in_markdown_are_read_and_edited_in_place() {
    let d = mermaid::parse_in_markdown(DOC, 1).unwrap().diagram;
    assert_eq!(d.edges.len(), 1);
    let out = mermaid::apply_in_markdown(
        DOC,
        2,
        &Edit::SetLabel {
            id: "nested".into(),
            label: "Nested node".into(),
        },
    )
    .unwrap();
    assert!(
        out.contains("  ```mermaid\n  flowchart LR\n    nested[Nested node]\n  ```"),
        "{out}"
    );
    // Everything outside the block is untouched.
    let blocks = markdown::mermaid_blocks(DOC);
    assert!(out.starts_with(&DOC[..blocks[2].range.start]));
    assert!(out.ends_with(&DOC[blocks[2].range.end..]));
    assert!(mermaid::apply_in_markdown(DOC, 9, &Edit::RemoveNode { id: "a".into() }).is_err());
}
