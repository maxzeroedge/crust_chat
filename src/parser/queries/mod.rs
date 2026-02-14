pub mod rust;
pub mod python;
pub mod javascript;
pub mod typescript;
pub mod java;
pub mod go;
pub mod c_cpp;

use super::CodeLanguage;

pub fn get_query_for_language(lang: CodeLanguage) -> &'static str {
    match lang {
        CodeLanguage::Rust => rust::QUERY,
        CodeLanguage::Python => python::QUERY,
        CodeLanguage::JavaScript => javascript::QUERY,
        CodeLanguage::TypeScript => typescript::QUERY,
        CodeLanguage::Java => java::QUERY,
        CodeLanguage::Go => go::QUERY,
        CodeLanguage::C => c_cpp::C_QUERY,
        CodeLanguage::Cpp => c_cpp::CPP_QUERY,
    }
}
