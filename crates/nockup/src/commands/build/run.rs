use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use colored::Colorize;
use tokio::process::Command;

use crate::manifest::NockAppManifest;

pub async fn run(project: String, bin: Option<String>, args: Vec<String>) -> Result<()> {
    // If project is ".", run the project in the current directory and read
    // nockapp.toml only for the display/package name.
    let (project_name, project_dir) = if project == "." {
        let cwd = std::env::current_dir()?;
        let manifest_path = cwd.join("nockapp.toml");

        if manifest_path.exists() {
            let manifest =
                NockAppManifest::load(&manifest_path).context("Failed to parse nockapp.toml")?;
            (manifest.package.name.trim().to_string(), cwd)
        } else {
            (project, cwd)
        }
    } else {
        let project_dir = PathBuf::from(&project);
        (project, project_dir)
    };

    // Check if project directory exists
    if !project_dir.exists() {
        return Err(anyhow::anyhow!(
            "Project directory '{}' not found", project_name
        ));
    }

    // Check if Cargo.toml exists
    let cargo_toml = project_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(anyhow::anyhow!("No Cargo.toml found in '{}'", project_name));
    }

    let binary_names = cargo_binary_names(&cargo_toml).await?;
    if bin.is_none() && binary_names.len() > 1 {
        let list = binary_names
            .iter()
            .map(|name| format!("  - {}", name))
            .collect::<Vec<_>>()
            .join("\n");
        let examples = binary_names
            .iter()
            .map(|name| format!("  nockup project run {} --bin {}", project_name, name))
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::bail!(
            "Project '{}' has multiple binaries. Choose one with --bin.\n\nAvailable binaries:\n{}\n\nExamples:\n{}",
            project_name,
            list,
            examples
        );
    }

    if let Some(selected_bin) = &bin {
        if !binary_names.is_empty() && !binary_names.contains(selected_bin) {
            let available = binary_names.join(", ");
            anyhow::bail!(
                "Binary '{}' not found in project '{}'. Available binaries: {}", selected_bin,
                project_name, available
            );
        }
    }

    match &bin {
        Some(selected_bin) => println!(
            "{} Running project '{}' binary '{}'...",
            "🔨".green(),
            project_name.cyan(),
            selected_bin.cyan()
        ),
        None => println!(
            "{} Running project '{}'...",
            "🔨".green(),
            project_name.cyan()
        ),
    }

    // Run cargo run in the project directory
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("--release") // Run in release mode by default
        .current_dir(&project_dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(selected_bin) = &bin {
        command.arg("--bin").arg(selected_bin);
    }

    // Add separator and pass through additional args to the selected program.
    if !args.is_empty() {
        command.arg("--").args(&args);
    }

    let status = command
        .status()
        .await
        .context("Failed to execute cargo run")?;

    if status.success() {
        println!("{} Run completed successfully!", "✓".green());
    } else {
        return Err(anyhow::anyhow!(
            "Run failed with exit code: {}",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(())
}

async fn cargo_binary_names(cargo_toml: &Path) -> Result<Vec<String>> {
    let cargo_toml_content = tokio::fs::read_to_string(cargo_toml)
        .await
        .with_context(|| format!("Failed to read {}", cargo_toml.display()))?;
    let cargo_toml_parsed: toml::Value = toml::from_str(&cargo_toml_content)
        .with_context(|| format!("Failed to parse {}", cargo_toml.display()))?;

    let Some(bins) = cargo_toml_parsed.get("bin") else {
        return Ok(Vec::new());
    };

    let bins = bins
        .as_array()
        .context("Invalid format for [[bin]] in Cargo.toml")?;
    Ok(bins
        .iter()
        .filter_map(|bin| bin.get("name").and_then(|name| name.as_str()))
        .map(ToString::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    fn write_multi_bin_project(root: &Path) {
        fs::create_dir_all(root.join("src")).expect("create src dir");
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "multi-bin-demo"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "listen"
path = "src/listen.rs"

[[bin]]
name = "talk"
path = "src/talk.rs"
"#,
        )
        .expect("write Cargo.toml");
    }

    #[tokio::test]
    async fn cargo_binary_names_reads_multiple_bins() {
        let temp = tempdir().expect("tempdir");
        write_multi_bin_project(temp.path());

        let bins = cargo_binary_names(&temp.path().join("Cargo.toml"))
            .await
            .expect("read bins");

        assert_eq!(bins, vec!["listen".to_string(), "talk".to_string()]);
    }

    #[tokio::test]
    async fn run_requires_bin_for_multi_binary_project() {
        let temp = tempdir().expect("tempdir");
        write_multi_bin_project(temp.path());

        let err = run(temp.path().display().to_string(), None, Vec::new())
            .await
            .expect_err("multi-bin run without --bin should fail before cargo run");
        let msg = err.to_string();

        assert!(msg.contains("has multiple binaries"));
        assert!(msg.contains("--bin listen"));
        assert!(msg.contains("--bin talk"));
    }

    #[tokio::test]
    async fn run_rejects_unknown_binary() {
        let temp = tempdir().expect("tempdir");
        write_multi_bin_project(temp.path());

        let err = run(
            temp.path().display().to_string(),
            Some("missing".to_string()),
            Vec::new(),
        )
        .await
        .expect_err("unknown --bin should fail before cargo run");
        let msg = err.to_string();

        assert!(msg.contains("Binary 'missing' not found"));
        assert!(msg.contains("listen, talk"));
    }
}
