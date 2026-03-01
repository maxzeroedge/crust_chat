use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::services::rag::chat_with_provider;

const MAX_FIX_ATTEMPTS: usize = 3;

const FIX_AGENT_PREAMBLE: &str = r#"You are a code fixing agent. You are given source code and compilation/build errors.
Your ONLY job is to fix the code so it compiles and runs successfully.

Rules:
- Output ONLY the complete fixed source code, nothing else
- Do NOT include markdown formatting (no ``` blocks)
- Fix all compilation errors reported
- Do not remove functionality — only fix errors
- If imports are missing, add them
- If types are wrong, correct them
- If a type or function is not found, it may have been renamed or removed in a newer version of the library — use the current/modern API
- Preserve the overall structure and intent of the code"#;

/// Supported project languages
#[derive(Debug, Clone, Copy)]
pub enum ProjectLang {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
}

impl ProjectLang {
    pub fn name(&self) -> &str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::Java => "java",
        }
    }

    pub fn extension(&self) -> &str {
        match self {
            Self::Rust => "rs",
            Self::Python => "py",
            Self::JavaScript => "js",
            Self::TypeScript => "ts",
            Self::Go => "go",
            Self::Java => "java",
        }
    }
}

/// Detect language from code content
pub fn detect_language_from_code(code: &str) -> ProjectLang {
    // Check for strong indicators in order of specificity
    if code.contains("fn main()") || code.contains("use std::") || code.contains("pub fn ")
        || code.contains("let mut ") || code.contains("-> Result<")
    {
        return ProjectLang::Rust;
    }
    if code.contains("func main()") || code.contains("package main") || code.contains("import \"fmt\"") {
        return ProjectLang::Go;
    }
    if code.contains("public static void main") || code.contains("System.out.println") {
        return ProjectLang::Java;
    }
    if code.contains("import ") && (code.contains("def ") || code.contains("print(")) {
        return ProjectLang::Python;
    }
    if code.contains(": string") || code.contains(": number") || code.contains("interface ") {
        return ProjectLang::TypeScript;
    }
    if code.contains("const ") || code.contains("require(") || code.contains("console.log") {
        return ProjectLang::JavaScript;
    }
    if code.contains("def ") || code.contains("class ") {
        return ProjectLang::Python;
    }

    // Default to Rust since this is a Rust-focused tool
    ProjectLang::Rust
}

/// Extract project name from code or use a default
fn extract_project_name(code: &str, lang: ProjectLang) -> String {
    match lang {
        ProjectLang::Go => {
            // Try to find package name
            for line in code.lines() {
                if let Some(pkg) = line.strip_prefix("package ") {
                    let name = pkg.trim().trim_end_matches(';');
                    if name != "main" {
                        return name.to_string();
                    }
                }
            }
            "hello_project".to_string()
        }
        ProjectLang::Java => {
            // Try to find public class name
            for line in code.lines() {
                if line.contains("public class ") {
                    if let Some(rest) = line.split("public class ").nth(1) {
                        let name = rest.split_whitespace().next().unwrap_or("Main");
                        return name.trim_end_matches('{').to_string();
                    }
                }
            }
            "Main".to_string()
        }
        _ => "hello_project".to_string(),
    }
}

/// Create project scaffolding based on language
fn scaffold_project(project_dir: &Path, lang: ProjectLang, code: &str) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(project_dir)?;
    let project_name = extract_project_name(code, lang);

    match lang {
        ProjectLang::Rust => {
            // Extract dependencies from use statements
            let deps = extract_rust_deps(code);
            let deps_toml = deps
                .iter()
                .map(|d| format!("{} = \"*\"", d))
                .collect::<Vec<_>>()
                .join("\n");

            let cargo_toml = format!(
                r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
{}
"#,
                project_name, deps_toml
            );

            std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;
            let src_dir = project_dir.join("src");
            std::fs::create_dir_all(&src_dir)?;
            let main_file = src_dir.join("main.rs");
            std::fs::write(&main_file, code)?;
            Ok(main_file)
        }
        ProjectLang::Python => {
            let main_file = project_dir.join("main.py");
            std::fs::write(&main_file, code)?;
            Ok(main_file)
        }
        ProjectLang::JavaScript => {
            let package_json = format!(
                r#"{{"name":"{}","version":"1.0.0","main":"index.js"}}"#,
                project_name
            );
            std::fs::write(project_dir.join("package.json"), package_json)?;
            let main_file = project_dir.join("index.js");
            std::fs::write(&main_file, code)?;
            Ok(main_file)
        }
        ProjectLang::TypeScript => {
            let tsconfig = r#"{"compilerOptions":{"target":"es2020","module":"commonjs","strict":true,"outDir":"./dist"}}"#;
            std::fs::write(project_dir.join("tsconfig.json"), tsconfig)?;
            let main_file = project_dir.join("index.ts");
            std::fs::write(&main_file, code)?;
            Ok(main_file)
        }
        ProjectLang::Go => {
            let go_mod = format!("module {}\n\ngo 1.21\n", project_name);
            std::fs::write(project_dir.join("go.mod"), go_mod)?;
            let main_file = project_dir.join("main.go");
            std::fs::write(&main_file, code)?;
            Ok(main_file)
        }
        ProjectLang::Java => {
            let main_file = project_dir.join(format!("{}.java", project_name));
            std::fs::write(&main_file, code)?;
            Ok(main_file)
        }
    }
}

/// Extract common Rust crate dependencies from use statements
fn extract_rust_deps(code: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let std_crates = [
        "std", "core", "alloc", "collections", "io", "fmt", "fs", "env", "path", "sync",
        "thread", "time", "net", "process",
    ];
    let skip_prefixes = ["self", "super", "crate"];

    let is_valid_crate = |name: &str| -> bool {
        !name.is_empty()
            && !std_crates.contains(&name)
            && !skip_prefixes.iter().any(|p| name.starts_with(p))
            && name.chars().all(|c| c.is_alphanumeric() || c == '_')
            // Crate names are lowercase/snake_case; skip PascalCase types like ControlFlow, Event
            && name.chars().next().map(|c| c.is_lowercase()).unwrap_or(false)
    };

    let mut in_use_block = false;
    let mut brace_depth: i32 = 0;

    for line in code.lines() {
        let trimmed = line.trim();

        // Track multi-line `use` blocks like `use winit::{ ... };`
        if trimmed.starts_with("use ") {
            let crate_name = trimmed
                .strip_prefix("use ")
                .unwrap()
                .split("::")
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(';');

            if is_valid_crate(crate_name) && !deps.contains(&crate_name.to_string()) {
                deps.push(crate_name.to_string());
            }

            // Check if this opens a multi-line block
            let opens = trimmed.chars().filter(|&c| c == '{').count() as i32;
            let closes = trimmed.chars().filter(|&c| c == '}').count() as i32;
            brace_depth = opens - closes;
            in_use_block = brace_depth > 0;
            continue;
        }

        if in_use_block {
            let opens = trimmed.chars().filter(|&c| c == '{').count() as i32;
            let closes = trimmed.chars().filter(|&c| c == '}').count() as i32;
            brace_depth += opens - closes;
            if brace_depth <= 0 {
                in_use_block = false;
            }
            continue;
        }

        // Scan code lines for path-style usage like `wgpu::Instance`, `tokio::spawn`
        for word in trimmed.split(|c: char| !c.is_alphanumeric() && c != '_' && c != ':') {
            if let Some((crate_part, _)) = word.split_once("::") {
                if is_valid_crate(crate_part) && !deps.contains(&crate_part.to_string()) {
                    deps.push(crate_part.to_string());
                }
            }
        }
    }

    // Map common crate renames
    deps.iter()
        .map(|d| match d.as_str() {
            "tokio" => "tokio".to_string(),
            "wgpu_types" => "wgpu-types".to_string(),
            "wgpu_hal" => "wgpu-hal".to_string(),
            _ => d.replace('_', "-"),
        })
        .collect()
}

/// Build/compile command for each language
fn build_command(project_dir: &Path, lang: ProjectLang) -> (String, Vec<String>) {
    match lang {
        ProjectLang::Rust => ("cargo".to_string(), vec!["check".to_string()]),
        ProjectLang::Python => (
            "python3".to_string(),
            vec!["-m".to_string(), "py_compile".to_string(), "main.py".to_string()],
        ),
        ProjectLang::JavaScript => ("node".to_string(), vec!["--check".to_string(), "index.js".to_string()]),
        ProjectLang::TypeScript => ("npx".to_string(), vec!["tsc".to_string(), "--noEmit".to_string()]),
        ProjectLang::Go => ("go".to_string(), vec!["build".to_string(), ".".to_string()]),
        ProjectLang::Java => {
            // Find the .java file
            let java_files: Vec<_> = std::fs::read_dir(project_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("java"))
                .collect();
            let file = java_files
                .first()
                .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
                .unwrap_or_else(|| "Main.java".to_string());
            ("javac".to_string(), vec![file])
        }
    }
}

/// Strip markdown fences from LLM output
fn strip_markdown_fences(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();

    if let Some(first) = lines.first() {
        if first.trim().starts_with("```") {
            lines.remove(0);
        }
    }
    if let Some(last) = lines.last() {
        if last.trim() == "```" {
            lines.pop();
        }
    }

    lines.join("\n")
}

/// Create a project from LLM response, scaffold, verify, and fix if needed.
/// Returns the project directory path on success.
pub async fn create_and_verify(
    llm_response: &str,
    output_dir: Option<&str>,
) -> anyhow::Result<PathBuf> {
    let total_start = Instant::now();

    // Step 1: Extract clean code from the response
    print!("Extracting code...           ");
    io::stdout().flush().ok();
    let start = Instant::now();

    let prompt = format!(
        "Extract the clean, runnable code from this text. Output a single file with all code combined:\n\n{}",
        llm_response
    );

    let code_preamble = r#"You are a code extraction agent. Output ONLY valid source code, nothing else.
Include all necessary imports. Remove markdown formatting, explanatory text, and citations.
Combine all code snippets into a single coherent, compilable file.
Do not output anything except code."#;

    let raw = chat_with_provider(&prompt, code_preamble, vec![]).await?;
    let code = strip_markdown_fences(&raw);
    println!("{:.2}s ({} chars)", start.elapsed().as_secs_f64(), code.len());

    if code.trim().is_empty() || code.trim() == "// No code found" {
        anyhow::bail!("No code found in the response.");
    }

    // Step 2: Detect language
    let lang = detect_language_from_code(&code);
    println!("Detected language:           {}", lang.name());

    // Step 3: Create project directory
    let project_dir = match output_dir {
        Some(dir) => PathBuf::from(dir),
        None => {
            let tmp = tempfile::Builder::new()
                .prefix("racl-project-")
                .tempdir()?;
            // Persist the temp dir so it's not deleted on drop
            tmp.keep()
        }
    };

    println!("Project directory:           {}", project_dir.display());

    // Step 4: Scaffold and write code
    print!("Scaffolding project...       ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let main_file = scaffold_project(&project_dir, lang, &code)?;
    println!("{:.2}s", start.elapsed().as_secs_f64());

    // Step 5: Build and verify (with fix loop)
    let mut current_code = code;
    let mut attempt = 0;

    loop {
        attempt += 1;
        print!("Build attempt {}/{}...        ", attempt, MAX_FIX_ATTEMPTS + 1);
        io::stdout().flush().ok();
        let start = Instant::now();

        let (cmd, args) = build_command(&project_dir, lang);
        let output = Command::new(&cmd)
            .args(&args)
            .current_dir(&project_dir)
            .output()?;

        let elapsed = start.elapsed().as_secs_f64();

        if output.status.success() {
            println!("{:.2}s - OK", elapsed);
            break;
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let errors = format!("{}\n{}", stderr, stdout);
        println!("{:.2}s - FAILED", elapsed);

        // Print first few lines of errors
        let error_preview: String = errors.lines().take(10).collect::<Vec<_>>().join("\n");
        println!("  Errors:\n  {}\n", error_preview.replace('\n', "\n  "));

        if attempt > MAX_FIX_ATTEMPTS {
            println!(
                "Could not fix after {} attempts. Project saved at: {}",
                MAX_FIX_ATTEMPTS,
                project_dir.display()
            );
            break;
        }

        // Step 6: Send code + errors to LLM for fixing
        print!("Fixing code...               ");
        io::stdout().flush().ok();
        let start = Instant::now();

        let fix_prompt = format!(
            "Fix this {} code. Here are the build errors:\n\n{}\n\nHere is the code:\n\n{}",
            lang.name(),
            errors,
            current_code
        );

        let fixed_raw = chat_with_provider(&fix_prompt, FIX_AGENT_PREAMBLE, vec![]).await?;
        let fixed_code = strip_markdown_fences(&fixed_raw);
        println!("{:.2}s", start.elapsed().as_secs_f64());

        // Write the fixed code
        std::fs::write(&main_file, &fixed_code)?;
        current_code = fixed_code.clone();

        // Regenerate Cargo.toml for Rust projects (deps may have changed)
        if matches!(lang, ProjectLang::Rust) {
            let deps = extract_rust_deps(&fixed_code);
            let deps_toml = deps.iter().map(|d| format!("{} = \"*\"", d)).collect::<Vec<_>>().join("\n");
            let project_name = extract_project_name(&fixed_code, lang);
            let cargo_toml = format!(
                "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n{}\n",
                project_name, deps_toml
            );
            std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;
        }
    }

    println!(
        "\nTotal: {:.2}s | Project: {}",
        total_start.elapsed().as_secs_f64(),
        project_dir.display()
    );

    Ok(project_dir)
}
