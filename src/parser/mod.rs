pub mod entity;
pub mod queries;
pub mod tree_sitter_parser;

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Java,
    Go,
    C,
    Cpp,
}

impl fmt::Display for CodeLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Java => "java",
            Self::Go => "go",
            Self::C => "c",
            Self::Cpp => "cpp",
        };
        write!(f, "{}", s)
    }
}

pub fn detect_language(file_path: &str) -> Option<CodeLanguage> {
    let ext = std::path::Path::new(file_path)
        .extension()?
        .to_str()?
        .to_lowercase();
    match ext.as_str() {
        "rs" => Some(CodeLanguage::Rust),
        "py" => Some(CodeLanguage::Python),
        "js" | "mjs" | "cjs" | "jsx" => Some(CodeLanguage::JavaScript),
        "ts" | "tsx" => Some(CodeLanguage::TypeScript),
        "java" => Some(CodeLanguage::Java),
        "go" => Some(CodeLanguage::Go),
        "c" | "h" => Some(CodeLanguage::C),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(CodeLanguage::Cpp),
        _ => None,
    }
}
