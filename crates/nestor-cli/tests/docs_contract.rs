use std::{fs, path::PathBuf};

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn cli_docs_crosslinks_and_command_sections_exist() -> TestResult<()> {
    let root = workspace_root()?;
    let docs = root.join("docs/cli");
    let readme = fs::read_to_string(docs.join("README.md"))?;
    for page in [
        "architecture.md",
        "progressive-disclosure.md",
        "commands.md",
        "workflows.md",
        "slots-and-json.md",
        "output-and-errors.md",
        "testing.md",
    ] {
        assert!(readme.contains(page), "README missing {page}");
        let body = fs::read_to_string(docs.join(page))?;
        assert!(
            body.contains("./README.md"),
            "{page} missing README backlink"
        );
    }

    let root_readme = fs::read_to_string(root.join("README.md"))?;
    assert!(root_readme.contains("docs/cli/README.md"));

    let commands = fs::read_to_string(docs.join("commands.md"))?;
    for heading in [
        "## Guide",
        "## Serve",
        "## Operational Commands",
        "## Chunk",
        "### Chunk Put",
        "### Chunk Get",
        "### Chunk Patch",
        "### Chunk Delete",
        "## Retrieve",
        "## Practice",
        "## Rehearse",
        "## Consolidate",
        "## Forget",
        "## Associate",
        "## Buffer",
        "### Buffer Set",
        "## Rule",
        "### Rule Eval",
    ] {
        assert!(commands.contains(heading), "commands.md missing {heading}");
    }

    for link in collect_relative_markdown_links(&docs)? {
        assert!(
            link.exists(),
            "missing linked markdown file: {}",
            link.display()
        );
    }
    Ok(())
}

fn workspace_root() -> TestResult<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| "could not resolve workspace root".into())
}

fn collect_relative_markdown_links(docs: &PathBuf) -> TestResult<Vec<PathBuf>> {
    let mut links = Vec::new();
    for entry in fs::read_dir(docs)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let body = fs::read_to_string(&path)?;
        for raw in body
            .split('(')
            .skip(1)
            .filter_map(|part| part.split(')').next())
        {
            if raw.starts_with("http") || raw.starts_with('#') || raw.starts_with("`") {
                continue;
            }
            let target = raw.split('#').next().unwrap_or(raw);
            if target.ends_with(".md") {
                let resolved = path
                    .parent()
                    .map(|parent| parent.join(target))
                    .ok_or_else(|| "markdown file had no parent".to_string())?;
                links.push(resolved);
            }
        }
    }
    Ok(links)
}
