//! Import the Mermaid flowchart subset agents and people write most: node shapes, labelled links,
//! chains, `&` fan-out and subgraphs (which become groups). Styling directives are ignored.
use crate::{
    diagram::{self, Diagram, Edge, Group, Node, Style},
    error::{AppError, Result},
};
use std::collections::HashMap;

struct Builder {
    nodes: Vec<Node>,
    index: HashMap<String, usize>,
    edges: Vec<Edge>,
    groups: Vec<Group>,
    /// Open subgraphs, innermost last, as indexes into `groups`.
    stack: Vec<usize>,
    grouped: std::collections::HashSet<String>,
}
impl Builder {
    fn node(&mut self, id: &str, label: Option<String>, shape: Option<&str>) {
        let index = match self.index.get(id) {
            Some(&i) => i,
            None => {
                self.nodes.push(Node {
                    id: id.into(),
                    kind: "module".into(),
                    label: id.into(),
                    ..Node::default()
                });
                self.index.insert(id.into(), self.nodes.len() - 1);
                self.nodes.len() - 1
            }
        };
        let node = &mut self.nodes[index];
        if let Some(label) = label.filter(|l| !l.is_empty()) {
            node.label = label.chars().take(200).collect();
        }
        if let Some(shape) = shape {
            node.kind = match shape {
                "cylinder" => "store",
                "diamond" => "decision",
                "circle" => "actor",
                _ => "module",
            }
            .into();
            if shape != "box" {
                node.style = Some(Style {
                    color: None,
                    shape: Some(shape.into()),
                });
            }
        }
        if let Some(&group) = self.stack.last() {
            if self.grouped.insert(id.to_owned()) {
                self.groups[group].node_ids.push(id.to_owned());
            }
        }
    }
}

pub fn parse(title: &str, text: &str) -> Result<Diagram> {
    let mut lines = text
        .lines()
        .map(|l| l.split("%%").next().unwrap_or("").trim())
        .filter(|l| !l.is_empty() && !l.starts_with("```"));
    let header = lines.next().unwrap_or_default();
    let keyword = header.split_whitespace().next().unwrap_or_default();
    if !["flowchart", "graph"].contains(&keyword) {
        return Err(AppError::Validation(
            "Only Mermaid flowcharts (`flowchart LR` or `graph TD`) can be imported.".into(),
        ));
    }
    let mut b = Builder {
        nodes: vec![],
        index: HashMap::new(),
        edges: vec![],
        groups: vec![],
        stack: vec![],
        grouped: Default::default(),
    };
    for line in lines {
        for statement in line.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            let word = statement.split_whitespace().next().unwrap_or_default();
            match word {
                "classDef" | "class" | "style" | "linkStyle" | "click" | "direction" => continue,
                "end" if statement == "end" => {
                    b.stack.pop();
                    continue;
                }
                "subgraph" => {
                    let rest = statement["subgraph".len()..].trim();
                    let (id, name) = match rest.find('[') {
                        Some(i) => (
                            rest[..i].trim().to_owned(),
                            unquote(rest[i + 1..].trim_end_matches(']')),
                        ),
                        None => (diagram::slugify(rest), unquote(rest)),
                    };
                    let mut id = if id.is_empty() {
                        diagram::slugify(&name)
                    } else {
                        id
                    };
                    while b.groups.iter().any(|g| g.id == id) {
                        id.push('_');
                    }
                    b.groups.push(Group {
                        id,
                        name: if name.is_empty() {
                            "Group".into()
                        } else {
                            name.chars().take(80).collect()
                        },
                        node_ids: vec![],
                        color: crate::domain::default_group_color(),
                    });
                    b.stack.push(b.groups.len() - 1);
                    continue;
                }
                _ => {}
            }
            statement_into(&mut b, statement)?;
        }
    }
    if b.nodes.is_empty() {
        return Err(AppError::Validation(
            "The Mermaid text has no nodes to import.".into(),
        ));
    }
    b.groups.retain(|g| !g.node_ids.is_empty());
    let mut diagram = Diagram {
        version: diagram::VERSION,
        kind: "flow".into(),
        title: title.trim().to_owned(),
        nodes: b.nodes,
        edges: b.edges,
        groups: b.groups,
        ..Diagram::default()
    };
    diagram.normalize();
    diagram.validate()?;
    Ok(diagram)
}

fn unquote(text: &str) -> String {
    text.trim()
        .trim_matches('"')
        .replace("<br>", " ")
        .replace("<br/>", " ")
        .trim()
        .to_owned()
}

struct Scanner<'a> {
    chars: Vec<char>,
    pos: usize,
    text: &'a str,
}
impl Scanner<'_> {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }
    fn skip_ws(&mut self) {
        while self.peek(0).is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }
    fn starts(&self, s: &str) -> bool {
        s.chars().enumerate().all(|(i, c)| self.peek(i) == Some(c))
    }
    fn error(&self) -> AppError {
        AppError::Validation(format!(
            "Cannot read Mermaid statement `{}`.",
            self.text.chars().take(120).collect::<String>()
        ))
    }
    fn id(&mut self) -> Option<String> {
        let start = self.pos;
        while let Some(c) = self.peek(0) {
            let continues = c.is_alphanumeric()
                || c == '_'
                || ((c == '-' || c == '.') && self.peek(1).is_some_and(char::is_alphanumeric));
            if !continues {
                break;
            }
            self.pos += 1;
        }
        (self.pos > start).then(|| self.chars[start..self.pos].iter().collect())
    }
    /// The label and shape after an id, e.g. `[(Orders DB)]` → ("Orders DB", "cylinder").
    fn shape(&mut self) -> Result<Option<(String, &'static str)>> {
        const SHAPES: [(&str, &str, &str); 10] = [
            ("([", "])", "pill"),
            ("[(", ")]", "cylinder"),
            ("((", "))", "circle"),
            ("[[", "]]", "box"),
            ("{{", "}}", "hexagon"),
            ("[/", "/]", "box"),
            ("[", "]", "box"),
            ("(", ")", "pill"),
            ("{", "}", "diamond"),
            (">", "]", "box"),
        ];
        for (open, close, shape) in SHAPES {
            if self.starts(open) {
                self.pos += open.chars().count();
                let start = self.pos;
                let mut quoted = false;
                while self.pos < self.chars.len() {
                    if self.peek(0) == Some('"') {
                        quoted = !quoted;
                    } else if !quoted && self.starts(close) {
                        let label: String = self.chars[start..self.pos].iter().collect();
                        self.pos += close.chars().count();
                        return Ok(Some((unquote(&label), shape)));
                    }
                    self.pos += 1;
                }
                return Err(self.error());
            }
        }
        Ok(None)
    }
    /// `A[x] & B` → the ids, declaring each node.
    fn endpoints(&mut self, b: &mut Builder) -> Result<Vec<String>> {
        let mut ids = vec![];
        loop {
            self.skip_ws();
            let id = self.id().ok_or_else(|| self.error())?;
            let shape = self.shape()?;
            match shape {
                Some((label, shape)) => b.node(&id, Some(label), Some(shape)),
                None => b.node(&id, None, None),
            }
            ids.push(id);
            self.skip_ws();
            if self.peek(0) == Some('&') {
                self.pos += 1;
                continue;
            }
            return Ok(ids);
        }
    }
    /// A link operator with its optional label, or None at the end of the statement.
    fn link(&mut self) -> Result<Option<(Option<String>, Option<&'static str>)>> {
        self.skip_ws();
        if self.pos >= self.chars.len() {
            return Ok(None);
        }
        let start = self.pos;
        while self.peek(0).is_some_and(|c| "-.=<>".contains(c)) {
            self.pos += 1;
        }
        if self.peek(0).is_some_and(|c| c == 'o' || c == 'x')
            && self.peek(1).is_some_and(char::is_whitespace)
            && self.pos > start
        {
            self.pos += 1;
        }
        let op: String = self.chars[start..self.pos].iter().collect();
        if op.len() < 2 {
            return Err(self.error());
        }
        let kind = if op.contains('.') {
            Some("async")
        } else if op.contains('=') {
            Some("critical")
        } else {
            None
        };
        let mut label = None;
        // `-- text -->`: an opening half followed by text, then the closing arrow.
        if ["--", "-.", "=="].contains(&op.as_str()) {
            let rest: String = self.chars[self.pos..].iter().collect();
            let closers = ["-->", "---", ".->", "-.-", "==>", "===", "--o", "--x"];
            let end = closers
                .iter()
                .filter_map(|c| rest.find(c).map(|i| (i, *c)))
                .min_by_key(|(i, _)| *i)
                .ok_or_else(|| self.error())?;
            label = Some(unquote(rest[..end.0].trim_end_matches(['-', '.', '='])));
            self.pos += rest[..end.0 + end.1.len()].chars().count();
            while self.peek(0).is_some_and(|c| "-.=>".contains(c)) {
                self.pos += 1;
            }
        }
        self.skip_ws();
        if self.peek(0) == Some('|') {
            self.pos += 1;
            let start = self.pos;
            while self.peek(0).is_some_and(|c| c != '|') {
                self.pos += 1;
            }
            if self.peek(0) != Some('|') {
                return Err(self.error());
            }
            label = Some(unquote(
                &self.chars[start..self.pos].iter().collect::<String>(),
            ));
            self.pos += 1;
        }
        Ok(Some((label.filter(|l| !l.is_empty()), kind)))
    }
}

fn statement_into(b: &mut Builder, text: &str) -> Result<()> {
    let mut s = Scanner {
        chars: text.chars().collect(),
        pos: 0,
        text,
    };
    let mut from = s.endpoints(b)?;
    while let Some((label, kind)) = s.link()? {
        let to = s.endpoints(b)?;
        for source in &from {
            for target in &to {
                b.edges.push(Edge {
                    source: source.clone(),
                    target: target.clone(),
                    label: label.clone().map(|l| l.chars().take(200).collect()),
                    kind: kind.map(str::to_owned),
                    ..Edge::default()
                });
            }
        }
        from = to;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_shapes_links_chains_and_subgraphs() {
        let d = parse(
            "Checkout",
            "```mermaid\nflowchart LR\n  %% comment\n  subgraph backend [Back end]\n    api([API]) -->|writes| db[(Orders DB)]\n  end\n  web[\"Web app\"] --> api & worker{Queue?}\n  worker -. retries .-> api\n  api == hot ==> cache((Cache)); classDef x fill:#fff\n  A-->B---C\n```",
        )
        .unwrap();
        let label = |id: &str| d.nodes.iter().find(|n| n.id == id).unwrap().label.clone();
        assert_eq!(label("db"), "Orders DB");
        assert_eq!(label("web"), "Web app");
        assert_eq!(d.nodes.iter().find(|n| n.id == "db").unwrap().kind, "store");
        assert_eq!(d.groups[0].name, "Back end");
        assert_eq!(d.groups[0].node_ids, ["api", "db"]);
        let edge = |s: &str, t: &str| {
            d.edges
                .iter()
                .find(|e| e.source == s && e.target == t)
                .unwrap()
        };
        assert_eq!(edge("api", "db").label.as_deref(), Some("writes"));
        assert!(d
            .edges
            .iter()
            .any(|e| e.source == "web" && e.target == "worker"));
        assert_eq!(edge("worker", "api").label.as_deref(), Some("retries"));
        assert_eq!(edge("worker", "api").kind.as_deref(), Some("async"));
        assert_eq!(edge("api", "cache").label.as_deref(), Some("hot"));
        assert!(d.edges.iter().any(|e| e.source == "B" && e.target == "C"));
        assert_eq!(d.kind, "flow");
    }

    #[test]
    fn refuses_other_diagram_types_and_garbage() {
        assert!(parse("x", "sequenceDiagram\nA->>B: hi").is_err());
        assert!(parse("x", "graph TD\nA[unclosed").is_err());
        assert!(parse("x", "graph TD\n").is_err());
    }
}
