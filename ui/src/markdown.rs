//! Agent output is untrusted: allow document formatting, never executable HTML or remote images.
use pulldown_cmark::{html, Options, Parser};
use std::collections::{HashMap, HashSet};

pub fn render_markdown(text: &str) -> String {
    let parser = Parser::new_ext(text, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH);
    let mut raw = String::new();
    html::push_html(&mut raw, parser);
    let mut attributes: HashMap<&str, HashSet<&str>> = HashMap::new();
    attributes.insert("a", HashSet::from(["href", "title"]));
    attributes.insert("ol", HashSet::from(["start"]));
    ammonia::Builder::default()
        .tags(HashSet::from([
            "p", "br", "strong", "em", "del", "blockquote", "pre", "code", "ul", "ol", "li", "h1", "h2", "h3", "h4", "h5",
            "h6", "hr", "table", "thead", "tbody", "tr", "th", "td", "a",
        ]))
        .generic_attributes(HashSet::new())
        .tag_attributes(attributes)
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .link_rel(Some("noopener noreferrer"))
        .set_tag_attribute_value("a", "target", "_blank")
        .clean(&raw)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_formatting_and_drops_scripts_and_images() {
        let html = render_markdown("# Title\n\nSome **bold** text <script>alert(1)</script> ![x](https://e.com/i.png)\n\n[link](https://example.com)");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("<img"));
        assert!(html.contains(r#"href="https://example.com""#));
        assert!(html.contains("noopener"));
        assert!(html.contains(r#"target="_blank""#));
    }
    #[test]
    fn blocks_javascript_urls() {
        let html = render_markdown("[x](javascript:alert(1))");
        assert!(!html.contains("javascript:"));
    }
}
