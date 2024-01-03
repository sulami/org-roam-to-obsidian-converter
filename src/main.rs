use std::{
    collections::HashMap,
    io::{stdin, stdout, Write},
    ops::DerefMut,
    path::Path,
    process::Stdio,
};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use indicatif::{style::ProgressStyle, ProgressBar};
use regex::{Captures, Regex};
use sqlx::{query_as, sqlite::SqliteConnectOptions, SqlitePool};
use tokio::process::Command;

#[derive(Parser)]
struct Args {
    /// Absolute location of the org-roam database
    #[clap(short, long)]
    db: String,

    /// Absolute location of the target directory
    #[clap(short, long)]
    target_dir: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    print!("This will write to your org-roam files. Make sure you have a backup. Continue? [y/N] ");
    stdout().flush()?;
    let mut input = String::new();
    stdin().read_line(&mut input)?;
    if &input != "y\n" {
        return Ok(());
    }

    print!("Collecting nodes...");
    let nodes = get_nodes(&args.db)
        .await
        .context("failed to load org-roam nodes")?;

    let progress_bar_style = "{msg} {bar:40.cyan/blue} {pos}/{len} | {eta} remaining";

    let progress_bar = ProgressBar::new(nodes.len() as u64)
        .with_message("Patching node links")
        .with_style(ProgressStyle::default_bar().template(progress_bar_style)?);
    for node in nodes.values() {
        patch_links(node, &nodes)
            .await
            .context("failed to patch links")?;
        progress_bar.inc(1);
    }

    progress_bar.reset();
    progress_bar.set_message("Exporting nodes");

    for node in nodes.values() {
        export(&args.target_dir, node)
            .await
            .context("failed to export node")?;
        progress_bar.inc(1);
    }
    progress_bar.finish_and_clear();

    Ok(())
}

#[derive(Clone, Debug, sqlx::FromRow)]
struct Node {
    id: String,
    file: String,
    level: i32,
    title: String,
}

impl Node {
    fn cleanup(&mut self) {
        self.title = self.title.replace('"', "");
        self.title = self.title.replace('/', " over ");
        self.file = self.file.replace('"', "");
        self.id = self.id.replace('"', "");
    }
}

/// Exports a node to Markdown.
async fn export(target_dir: &str, node: &Node) -> Result<()> {
    let target_file = format!("{target_dir}/{}.md", node.title);

    if Path::new(&target_file).exists() {
        return Ok(());
    }

    let subtree_only = if node.level == 0 { "nil" } else { "t" };
    let proc = Command::new("emacs")
        .args([
            "--batch",
            "-l",
            "~/.emacs.d/init.el",
            "--eval",
            &format!(
                r#"(progn
                 (message "Exporting {}")
                 (require 'ox-gfm)
                 (org-roam-node-open (org-roam-node-from-id "{}"))
                 (org-export-to-file 'gfm "{target_file}" nil {subtree_only}))"#,
                node.title, node.id
            ),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to run emacs")?
        .wait_with_output()
        .await?;

    if proc.status.success() {
        Ok(())
    } else {
        println!(
            "Failed to export {}:\n{}",
            node.title,
            String::from_utf8(proc.stderr).context("failed to render emacs' output")?
        );
        Err(anyhow!("Failed to export {}", node.title))
    }
}

/// Patches links in a node from [[id:<id>][<name>]] to [[<md-file>][<name>]].
async fn patch_links(node: &Node, nodes: &HashMap<String, Node>) -> Result<()> {
    let mut contents = tokio::fs::read_to_string(&node.file)
        .await
        .context("failed to read original file")?;

    let re = Regex::new(r"\[\[id:([0-9A-F-]+?)\]\[([^\]]+?)\]\]")?;
    // Find the file for the link in nodes.
    contents = re
        .replace_all(&contents, |caps: &Captures| {
            let id = caps.get(1).unwrap().as_str();
            let name = caps.get(2).unwrap().as_str();
            let target_node = nodes.get(id).unwrap();
            let target_file = format!("./{}.md", target_node.title.replace(' ', "%20"));
            format!("[[{target_file}][{name}]]")
        })
        .to_string();

    tokio::fs::write(&node.file, contents)
        .await
        .context("failed to save patched file")?;

    Ok(())
}

/// Gets all nodes from the org-roam DB and return them as a hashmap keyed by ID.
async fn get_nodes(db: &str) -> Result<HashMap<String, Node>> {
    let pool = SqlitePool::connect_with(SqliteConnectOptions::new().filename(db))
        .await
        .context("failed to open org-roam SQLite database")?;

    let mut rows = query_as::<_, Node>("SELECT id, file, level, title FROM nodes")
        .fetch_all(pool.acquire().await?.deref_mut())
        .await
        .context("failed to query org-roam SQLite database")?;

    for row in rows.iter_mut() {
        row.cleanup();
    }

    Ok(rows.iter().map(|n| (n.id.clone(), n.clone())).collect())
}
