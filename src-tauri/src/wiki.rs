//! The repository wiki the agent writes: an outline of the repository to plan pages from, and the
//! checks a page passes before it is proposed, so citations and diagrams in it always resolve.
use crate::repo::Repo;
use schematlas_parser::model::CodeRef;
use std::collections::{BTreeMap, HashSet};

/// Files the outline looks at; enough for any repository a person can document by hand.
const MAX_FILES: usize = 20_000;
/// Manifests and build files that tell what a repository is made of.
const MANIFESTS: [&str; 22] = [
    "Cargo.toml",
    "package.json",
    "pnpm-workspace.yaml",
    "go.mod",
    "pyproject.toml",
    "setup.py",
    "requirements.txt",
    "Gemfile",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "composer.json",
    "mix.exs",
    "Package.swift",
    "pubspec.yaml",
    "CMakeLists.txt",
    "Makefile",
    "Dockerfile",
    "docker-compose.yml",
    "tsconfig.json",
    "deno.json",
    "tauri.conf.json",
];
/// File names that usually start a program.
const ENTRY_POINTS: [&str; 14] = [
    "main.rs",
    "lib.rs",
    "main.go",
    "main.py",
    "__main__.py",
    "app.py",
    "manage.py",
    "index.ts",
    "index.js",
    "main.ts",
    "server.ts",
    "App.tsx",
    "Program.cs",
    "Main.java",
];

fn language(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "rs" => "Rust",
        "ts" | "tsx" | "mts" => "TypeScript",
        "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
        "py" => "Python",
        "go" => "Go",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "rb" => "Ruby",
        "php" => "PHP",
        "cs" => "C#",
        "c" | "h" => "C",
        "cc" | "cpp" | "hpp" | "cxx" => "C++",
        "swift" => "Swift",
        "ex" | "exs" => "Elixir",
        "scala" => "Scala",
        "dart" => "Dart",
        "sql" => "SQL",
        "vue" => "Vue",
        "svelte" => "Svelte",
        "css" | "scss" => "CSS",
        "html" => "HTML",
        "md" => "Markdown",
        "mmd" | "mermaid" => "Mermaid",
        "dbml" => "DBML",
        "proto" => "Protobuf",
        "graphql" | "gql" => "GraphQL",
        "tf" => "Terraform",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "json" => "JSON",
        _ => return None,
    })
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A compact picture of the repository for planning a wiki: its commit, languages, manifests,
/// entry points, README opening and folder tree (with file counts), in plain text.
pub fn outline(repo: &Repo) -> String {
    let files = repo.files(MAX_FILES);
    let mut out = String::new();
    let name = repo
        .root()
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    out.push_str(&format!("Repository {name}: {} files", files.len()));
    if files.len() >= MAX_FILES {
        out.push_str(" (listing stopped there)");
    }
    match repo.head_commit() {
        Some(commit) => out.push_str(&format!(", commit {commit}\n")),
        None => out.push_str(", not a git checkout\n"),
    }
    let mut languages: BTreeMap<&str, (usize, u64)> = BTreeMap::new();
    for (path, size) in &files {
        if let Some(lang) = language(path) {
            let slot = languages.entry(lang).or_default();
            slot.0 += 1;
            slot.1 += size;
        }
    }
    let mut ranked: Vec<_> = languages.into_iter().collect();
    ranked.sort_by_key(|(_, (_, size))| std::cmp::Reverse(*size));
    out.push_str("\nLanguages (files, KB): ");
    out.push_str(
        &ranked
            .iter()
            .take(12)
            .map(|(lang, (n, size))| format!("{lang} {n} / {}", size / 1024))
            .collect::<Vec<_>>()
            .join(", "),
    );
    out.push('\n');
    let pick = |names: &[&str]| -> Vec<&str> {
        files
            .iter()
            .map(|(p, _)| p.as_str())
            .filter(|p| names.contains(&file_name(p)))
            .filter(|p| p.matches('/').count() <= 4)
            .take(40)
            .collect()
    };
    let manifests = pick(&MANIFESTS);
    if !manifests.is_empty() {
        out.push_str(&format!("\nManifests: {}\n", manifests.join(", ")));
    }
    let entries = pick(&ENTRY_POINTS);
    if !entries.is_empty() {
        out.push_str(&format!("Likely entry points: {}\n", entries.join(", ")));
    }
    let existing: Vec<&str> = files
        .iter()
        .map(|(p, _)| p.as_str())
        .filter(|p| {
            p.starts_with(".schematlas/")
                && !p.starts_with(".schematlas/layout/")
                && (p.ends_with(".md") || p.ends_with(".mmd") || p.ends_with(".yaml"))
        })
        .take(60)
        .collect();
    if !existing.is_empty() {
        out.push_str(&format!(
            "Existing Schematlas pages and diagrams: {}\n",
            existing.join(", ")
        ));
    }
    let readme = files
        .iter()
        .map(|(p, _)| p.as_str())
        .find(|p| !p.contains('/') && file_name(p).to_ascii_lowercase().starts_with("readme"));
    if let Some(readme) = readme {
        if let Ok(Some((text, _))) = repo.read(readme) {
            let opening: Vec<&str> = text.lines().take(30).collect();
            out.push_str(&format!("\n{readme} (opening):\n{}\n", opening.join("\n")));
        }
    }
    // Folders to depth 3 with their file counts, largest first within each parent.
    let mut folders: BTreeMap<String, usize> = BTreeMap::new();
    for (path, _) in &files {
        let parts: Vec<&str> = path.split('/').collect();
        for depth in 1..parts.len().min(4) {
            *folders.entry(parts[..depth].join("/")).or_default() += 1;
        }
    }
    out.push_str("\nFolders (files inside):\n");
    for (shown, (folder, count)) in folders.iter().enumerate() {
        if shown >= 150 {
            out.push_str("…\n");
            break;
        }
        let depth = folder.matches('/').count();
        out.push_str(&format!("{}{folder}/ {count}\n", "  ".repeat(depth)));
    }
    let top: Vec<&str> = files
        .iter()
        .map(|(p, _)| p.as_str())
        .filter(|p| !p.contains('/'))
        .take(40)
        .collect();
    out.push_str(&format!("\nTop-level files: {}\n", top.join(", ")));
    out
}

/// Code references a page cites: `path:line`, `path:start-end` or `path#Lx-Ly`, in backticks or
/// as link targets, in order of appearance.
pub fn citations(markdown: &str) -> Vec<String> {
    let mut found: Vec<String> = vec![];
    let mut push = |candidate: &str| {
        let candidate = candidate.trim();
        let looks_like_ref = candidate.contains('/')
            && !candidate.contains(char::is_whitespace)
            && !candidate.contains("://")
            && (candidate.contains(':') || candidate.contains("#L"))
            && candidate
                .split([':', '#'])
                .next()
                .is_some_and(|p| p.contains('.'));
        if looks_like_ref && !found.iter().any(|f| f == candidate) {
            found.push(candidate.to_owned());
        }
    };
    for (i, part) in markdown.split('`').enumerate() {
        if i % 2 == 1 {
            push(part);
        }
    }
    for piece in markdown.split("](").skip(1) {
        if let Some(target) = piece.split(')').next() {
            push(target);
        }
    }
    found
}

/// Why a page cannot be proposed as written: citations that do not resolve, links to diagrams
/// that do not exist, Mermaid blocks that do not parse. Empty when the page is sound.
pub fn check_page(repo: &Repo, diagrams: &HashSet<String>, markdown: &str) -> Vec<String> {
    let mut problems = vec![];
    for cite in citations(markdown) {
        let code_ref = CodeRef::parse(&cite);
        match repo.snippet(&code_ref, 0) {
            Ok(snippet) => {
                if let Some(end) = code_ref.end_line.filter(|end| *end > snippet.total) {
                    problems.push(format!(
                        "`{cite}`: {} has {} lines, not {end}.",
                        code_ref.path, snippet.total
                    ));
                }
            }
            Err(e) => problems.push(format!("`{cite}`: {e}")),
        }
    }
    for piece in markdown.split("schematlas:diagram/").skip(1) {
        let id: String = piece
            .chars()
            .take_while(|c| !matches!(c, ')' | ' ' | '"' | '>' | '\n'))
            .collect();
        if !diagrams.contains(&id) && !diagrams.contains(&format!("{id}#0")) {
            problems.push(format!(
                "`schematlas:diagram/{id}` does not name a diagram; write it first with write_diagram or link one from list_diagrams."
            ));
        }
    }
    for block in schematlas_parser::markdown::mermaid_blocks(markdown) {
        if let Err(schematlas_parser::Error::Parse { message, .. }) =
            schematlas_parser::mermaid::parse(&block.source(markdown))
        {
            problems.push(format!(
                "Mermaid block {} does not parse: {message}",
                block.index + 1
            ));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn repo() -> (tempfile::TempDir, Repo) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/api")).unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"shop\"\n",
        )
        .unwrap();
        fs::write(dir.path().join("README.md"), "# Shop\n\nSells things.\n").unwrap();
        fs::write(
            dir.path().join("src/main.rs"),
            "fn main() {\n    serve();\n}\n",
        )
        .unwrap();
        fs::write(dir.path().join("src/api/mod.rs"), "pub fn serve() {}\n").unwrap();
        fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        fs::create_dir_all(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target/junk.rs"), "x").unwrap();
        fs::create_dir_all(dir.path().join(".git/refs/heads")).unwrap();
        fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            dir.path().join(".git/refs/heads/main"),
            "0123456789abcdef0123456789abcdef01234567\n",
        )
        .unwrap();
        let repo = Repo::open(dir.path().to_str().unwrap()).unwrap();
        (dir, repo)
    }

    #[test]
    fn the_outline_names_what_a_wiki_plan_needs() {
        let (_dir, repo) = repo();
        let text = outline(&repo);
        assert!(text.contains("commit 0123456789abcdef"), "{text}");
        assert!(text.contains("Rust 2"), "{text}");
        assert!(text.contains("Manifests: Cargo.toml"), "{text}");
        assert!(text.contains("src/main.rs"), "{text}");
        assert!(text.contains("Sells things."), "{text}");
        assert!(text.contains("src/api/ 1"), "{text}");
        assert!(!text.contains("target"), "ignored files stay out: {text}");
    }

    #[test]
    fn pages_are_checked_before_they_are_proposed() {
        let (_dir, repo) = repo();
        let diagrams = HashSet::from([".schematlas/diagrams/flow.mmd".to_string()]);
        let good = "# Shop\n\nStarts in `src/main.rs:1-3` and serves from [api](src/api/mod.rs:1).\n\n[Flow](schematlas:diagram/.schematlas/diagrams/flow.mmd)\n\n```mermaid\nflowchart LR\n  a --> b\n```\n\nRun `cargo test` and see `README.md`.\n";
        assert_eq!(check_page(&repo, &diagrams, good), Vec::<String>::new());
        let bad = "`src/main.rs:2-9` and `src/gone.rs:1`\n\n[x](schematlas:diagram/.schematlas/diagrams/nope.mmd)\n\n```mermaid\nflowchart LR\n  a[unclosed\n```\n";
        let problems = check_page(&repo, &diagrams, bad);
        assert_eq!(problems.len(), 4, "{problems:#?}");
        assert!(problems[0].contains("has 3 lines"), "{problems:#?}");
        assert!(problems[1].contains("does not exist"), "{problems:#?}");
        assert!(problems[2].contains("nope.mmd"), "{problems:#?}");
        assert!(problems[3].contains("Mermaid block 1"), "{problems:#?}");
    }

    #[test]
    fn citations_ignore_commands_and_prose() {
        assert_eq!(
            citations(
                "`cargo test`, `a/b.rs:3`, `x::y`, [doc](https://e.com/a.rs:1), `a/b.rs#L2-L4`"
            ),
            ["a/b.rs:3", "a/b.rs#L2-L4"]
        );
    }
}
