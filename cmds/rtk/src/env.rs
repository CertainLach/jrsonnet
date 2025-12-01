use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::spec::Environment;

/// Add a new environment at the given path
pub fn add_env(
    path: &str,
    server: Option<String>,
    context_names: Vec<String>,
    namespace: String,
    diff_strategy: Option<String>,
    inject_labels: bool,
    inline: bool,
) -> Result<()> {
    let path = PathBuf::from(path);
    let abs_path = if path.is_absolute() {
        path.clone()
    } else {
        std::env::current_dir()?.join(&path)
    };

    // Create directory if it doesn't exist
    if !abs_path.exists() {
        fs::create_dir_all(&abs_path)
            .with_context(|| format!("Failed to create directory: {}", abs_path.display()))?;
    } else {
        anyhow::bail!("Directory {} already exists", abs_path.display());
    }

    // Create environment config
    let mut env = Environment::new();
    env.spec.api_server = server;
    if !context_names.is_empty() {
        env.spec.context_names = Some(context_names);
    }
    env.spec.namespace = namespace;
    env.spec.diff_strategy = diff_strategy;
    if inject_labels {
        env.spec.inject_labels = Some(true);
    }

    // Set metadata name from path
    if let Some(name) = abs_path.file_name().and_then(|n| n.to_str()) {
        env.metadata.name = Some(name.to_string());
        env.metadata.namespace = Some(abs_path.to_string_lossy().to_string());
    }

    if inline {
        // Create inline environment (main.jsonnet with embedded environment)
        env.data = Some(serde_json::json!({}));
        let jsonnet_content = serde_json::to_string_pretty(&env)?;
        let main_path = abs_path.join("main.jsonnet");
        fs::write(&main_path, jsonnet_content)
            .with_context(|| format!("Failed to write {}", main_path.display()))?;

        println!("Environment created at: {}", abs_path.display());
        println!("Type: inline");
    } else {
        // Create static environment (spec.json + main.jsonnet)
        let spec_path = abs_path.join("spec.json");
        let spec_content = serde_json::to_string_pretty(&env)?;
        fs::write(&spec_path, spec_content)
            .with_context(|| format!("Failed to write {}", spec_path.display()))?;

        // Create empty main.jsonnet
        let main_path = abs_path.join("main.jsonnet");
        fs::write(&main_path, "{}\n")
            .with_context(|| format!("Failed to write {}", main_path.display()))?;

        println!("Environment created at: {}", abs_path.display());
        println!("Type: static");
        println!("\nFiles created:");
        println!("  - spec.json");
        println!("  - main.jsonnet");
    }

    Ok(())
}

/// Update an existing environment
pub fn set_env(
    path: &str,
    server: Option<String>,
    context_names: Vec<String>,
    namespace: Option<String>,
    diff_strategy: Option<String>,
    inject_labels: bool,
) -> Result<()> {
    let path = PathBuf::from(path);
    let abs_path = if path.is_absolute() {
        path.clone()
    } else {
        std::env::current_dir()?.join(&path)
    };

    if !abs_path.exists() {
        anyhow::bail!("Environment directory does not exist: {}", abs_path.display());
    }

    let spec_path = abs_path.join("spec.json");
    if !spec_path.exists() {
        anyhow::bail!(
            "spec.json not found in {}. Only static environments can be updated with 'set'",
            abs_path.display()
        );
    }

    // Read existing spec
    let spec_content = fs::read_to_string(&spec_path)
        .with_context(|| format!("Failed to read {}", spec_path.display()))?;
    let mut env: Environment = serde_json::from_str(&spec_content)
        .with_context(|| format!("Failed to parse {}", spec_path.display()))?;

    // Update fields
    let mut updated = false;

    if let Some(new_server) = server {
        if env.spec.api_server.as_ref() != Some(&new_server) {
            println!(
                "Updated spec.apiServer: {:?} -> {}",
                env.spec.api_server, new_server
            );
            env.spec.api_server = Some(new_server);
            updated = true;
        }
    }

    if !context_names.is_empty() {
        let new_contexts = Some(context_names.clone());
        if env.spec.context_names != new_contexts {
            println!(
                "Updated spec.contextNames: {:?} -> {:?}",
                env.spec.context_names, context_names
            );
            env.spec.context_names = new_contexts;
            updated = true;
        }
    }

    if let Some(new_namespace) = namespace {
        if env.spec.namespace != new_namespace {
            println!(
                "Updated spec.namespace: {} -> {}",
                env.spec.namespace, new_namespace
            );
            env.spec.namespace = new_namespace;
            updated = true;
        }
    }

    if let Some(new_diff_strategy) = diff_strategy {
        if env.spec.diff_strategy.as_ref() != Some(&new_diff_strategy) {
            println!(
                "Updated spec.diffStrategy: {:?} -> {}",
                env.spec.diff_strategy, new_diff_strategy
            );
            env.spec.diff_strategy = Some(new_diff_strategy);
            updated = true;
        }
    }

    if inject_labels {
        if env.spec.inject_labels != Some(true) {
            println!("Updated spec.injectLabels: {:?} -> true", env.spec.inject_labels);
            env.spec.inject_labels = Some(true);
            updated = true;
        }
    }

    if updated {
        // Write back the updated spec
        let spec_content = serde_json::to_string_pretty(&env)?;
        fs::write(&spec_path, spec_content)
            .with_context(|| format!("Failed to write {}", spec_path.display()))?;
        println!("\nEnvironment updated successfully");
    } else {
        println!("No changes made");
    }

    Ok(())
}

/// List environments in the given path
pub fn list_envs(path: Option<String>) -> Result<()> {
    let search_path = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        std::env::current_dir()?
    };

    println!("NAME                    NAMESPACE          SERVER");
    println!("────────────────────────────────────────────────────────────────");

    // For now, just check if current directory is an environment
    // In a full implementation, this would recursively search for environments
    if let Ok(env) = load_env(&search_path) {
        let name = env.metadata.name.unwrap_or_else(|| "unnamed".to_string());
        let namespace = env.spec.namespace;
        let server = env.spec.api_server.unwrap_or_else(|| "-".to_string());
        println!("{:<24}{:<19}{}", name, namespace, server);
    } else {
        println!("No environments found in {}", search_path.display());
    }

    Ok(())
}

/// Remove an environment
pub fn remove_env(path: &str) -> Result<()> {
    let path = PathBuf::from(path);
    let abs_path = if path.is_absolute() {
        path.clone()
    } else {
        std::env::current_dir()?.join(&path)
    };

    if !abs_path.exists() {
        anyhow::bail!("Environment directory does not exist: {}", abs_path.display());
    }

    // Confirm deletion
    print!("Permanently removing the environment located at '{}'. Type 'yes' to confirm: ", abs_path.display());
    use std::io::{self, Write};
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim() != "yes" {
        println!("Aborted");
        return Ok(());
    }

    fs::remove_dir_all(&abs_path)
        .with_context(|| format!("Failed to remove directory: {}", abs_path.display()))?;

    println!("Removed {}", abs_path.display());

    Ok(())
}

/// Load an environment from a directory
fn load_env(path: &Path) -> Result<Environment> {
    let spec_path = path.join("spec.json");

    if spec_path.exists() {
        // Static environment
        let content = fs::read_to_string(&spec_path)?;
        let env: Environment = serde_json::from_str(&content)?;
        Ok(env)
    } else {
        // Try inline environment
        let main_path = path.join("main.jsonnet");
        if main_path.exists() {
            // For now, just return an error - full implementation would parse jsonnet
            anyhow::bail!("Inline environments not yet fully supported")
        } else {
            anyhow::bail!("Not a valid environment directory")
        }
    }
}
