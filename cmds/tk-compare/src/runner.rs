use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct RunResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

pub fn run_command(executable: &str, args: &[String], workspace_dir: Option<&str>, working_dir: Option<&str>) -> Result<RunResult> {
    let start = Instant::now();
    
    let mut cmd = ProcessCommand::new(executable);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    
    // Determine the actual working directory
    let actual_working_dir = match (workspace_dir, working_dir) {
        (Some(ws), Some(wd)) => {
            // If both are specified, combine them: workspace_dir/working_dir
            let combined = format!("{}/{}", ws, wd);
            std::fs::create_dir_all(&combined)?;
            Some(combined)
        }
        (Some(ws), None) => {
            // Only workspace directory
            std::fs::create_dir_all(ws)?;
            Some(ws.to_string())
        }
        (None, Some(wd)) => {
            // Only working directory (no workspace isolation)
            Some(wd.to_string())
        }
        (None, None) => None,
    };
    
    if let Some(dir) = &actual_working_dir {
        cmd.current_dir(dir);
    }
    
    let output = cmd.output()
        .with_context(|| format!("Failed to execute command: {} {:?}", executable, args))?;
    
    let duration = start.elapsed();
    
    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    
    Ok(RunResult {
        exit_code,
        stdout,
        stderr,
        duration,
    })
}

pub fn compare_directories(dir1: &str, dir2: &str) -> Result<bool> {
    // Get all files recursively from both directories
    let files1 = collect_files(dir1)?;
    let files2 = collect_files(dir2)?;
    
    // Compare file sets
    if files1.keys().collect::<Vec<_>>() != files2.keys().collect::<Vec<_>>() {
        return Ok(false);
    }
    
    // Compare file contents
    for (path, content1) in &files1 {
        if let Some(content2) = files2.get(path) {
            if content1 != content2 {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }
    
    Ok(true)
}

fn collect_files(dir: &str) -> Result<HashMap<String, Vec<u8>>> {
    use std::fs;
    use std::collections::HashMap;
    
    let mut files = HashMap::new();
    let base_path = PathBuf::from(dir);
    
    if !base_path.exists() {
        return Ok(files);
    }
    
    fn visit_dirs(dir: &PathBuf, base: &PathBuf, files: &mut HashMap<String, Vec<u8>>) -> Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, base, files)?;
                } else {
                    let relative_path = path.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .to_string();
                    let content = fs::read(&path)?;
                    files.insert(relative_path, content);
                }
            }
        }
        Ok(())
    }
    
    visit_dirs(&base_path, &base_path, &mut files)?;
    Ok(files)
}

use std::collections::HashMap;

