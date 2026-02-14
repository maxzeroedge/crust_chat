use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityType {
    File,
    Module,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Function,
    Method,
    Variable,
    Constant,
    Import,
    TypeAlias,
}

impl fmt::Display for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::File => "file",
            Self::Module => "module",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Function => "function",
            Self::Method => "method",
            Self::Variable => "variable",
            Self::Constant => "constant",
            Self::Import => "import",
            Self::TypeAlias => "type_alias",
        };
        write!(f, "{}", s)
    }
}

impl EntityType {
    pub fn neo4j_label(&self) -> &str {
        match self {
            Self::File => "File",
            Self::Module => "Module",
            Self::Class => "Class",
            Self::Struct => "Struct",
            Self::Enum => "Enum",
            Self::Interface => "Interface",
            Self::Trait => "Trait",
            Self::Function => "Function",
            Self::Method => "Method",
            Self::Variable => "Variable",
            Self::Constant => "Constant",
            Self::Import => "Import",
            Self::TypeAlias => "TypeAlias",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipType {
    Contains,
    Calls,
    Imports,
    Inherits,
    Implements,
    TypeReference,
    Uses,
}

impl RelationshipType {
    pub fn neo4j_type(&self) -> &str {
        match self {
            Self::Contains => "CONTAINS",
            Self::Calls => "CALLS",
            Self::Imports => "IMPORTS",
            Self::Inherits => "INHERITS",
            Self::Implements => "IMPLEMENTS",
            Self::TypeReference => "TYPE_REFERENCE",
            Self::Uses => "USES",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CodeEntity {
    pub qualified_name: String,
    pub name: String,
    pub entity_type: EntityType,
    pub language: super::CodeLanguage,
    pub source_file: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub parent: Option<String>,
    pub signature: String,
}

#[derive(Debug, Clone)]
pub struct CodeRelationship {
    pub from_qualified_name: String,
    pub to_qualified_name: String,
    pub relationship_type: RelationshipType,
}

pub struct ParseResult {
    pub entities: Vec<CodeEntity>,
    pub relationships: Vec<CodeRelationship>,
}
