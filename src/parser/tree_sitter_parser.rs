use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

use super::CodeLanguage;
use super::entity::*;
use super::queries;

fn get_language(lang: CodeLanguage) -> Language {
    match lang {
        CodeLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
        CodeLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        CodeLanguage::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
        CodeLanguage::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        CodeLanguage::Java => tree_sitter_java::LANGUAGE.into(),
        CodeLanguage::Go => tree_sitter_go::LANGUAGE.into(),
        CodeLanguage::C => tree_sitter_c::LANGUAGE.into(),
        CodeLanguage::Cpp => tree_sitter_cpp::LANGUAGE.into(),
    }
}

/// Parse a source file and extract code entities and relationships.
pub fn parse_file(file_path: &str, source: &str, lang: CodeLanguage) -> anyhow::Result<ParseResult> {
    let ts_language = get_language(lang);
    let mut parser = Parser::new();
    parser.set_language(&ts_language)?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", file_path))?;

    let query_str = queries::get_query_for_language(lang);
    let query = Query::new(&ts_language, query_str)?;
    let mut cursor = QueryCursor::new();

    let capture_names = query.capture_names();

    let mut entities: Vec<CodeEntity> = Vec::new();
    let mut relationships: Vec<CodeRelationship> = Vec::new();

    // Track the file itself as an entity
    let file_qn = file_path.to_string();
    entities.push(CodeEntity {
        qualified_name: file_qn.clone(),
        name: std::path::Path::new(file_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| file_path.to_string()),
        entity_type: EntityType::File,
        language: lang,
        source_file: file_path.to_string(),
        content: String::new(), // Don't embed the whole file
        start_line: 1,
        end_line: source.lines().count(),
        parent: None,
        signature: file_path.to_string(),
    });

    // Process query matches
    let mut matches_iter = cursor.matches(&query, tree.root_node(), source.as_bytes());

    while let Some(m) = matches_iter.next() {
        for capture in m.captures {
            let capture_name = capture_names[capture.index as usize];
            let node = capture.node;
            let text = node
                .utf8_text(source.as_bytes())
                .unwrap_or("")
                .to_string();
            let start_line = node.start_position().row + 1;
            let end_line = node.end_position().row + 1;

            match capture_name {
                // Entity definitions (the full node)
                "function.def" | "struct.def" | "enum.def" | "trait.def"
                | "class.def" | "interface.def" | "method.def" | "constant.def"
                | "static.def" | "variable.def" | "import.def" | "import.from_def"
                | "type_alias.def" | "module.def" => {
                    // We handle these via their .name captures below
                }

                // Entity name captures — create entities
                name_capture
                    if name_capture.ends_with(".name")
                        && !name_capture.starts_with("call.")
                        && !name_capture.starts_with("inherits.")
                        && !name_capture.starts_with("implements.") =>
                {
                    let prefix = name_capture.strip_suffix(".name").unwrap();
                    let entity_type = match prefix {
                        "function" => EntityType::Function,
                        "struct" => EntityType::Struct,
                        "enum" => EntityType::Enum,
                        "trait" => EntityType::Trait,
                        "class" => EntityType::Class,
                        "interface" => EntityType::Interface,
                        "method" => EntityType::Method,
                        "constant" => EntityType::Constant,
                        "static" => EntityType::Variable,
                        "variable" => EntityType::Variable,
                        "type_alias" => EntityType::TypeAlias,
                        "module" => EntityType::Module,
                        _ => continue,
                    };

                    // Find the parent def node to get full content
                    let def_capture_name = format!("{}.def", prefix);
                    let def_node = m.captures.iter().find(|c| {
                        capture_names[c.index as usize] == def_capture_name
                    });

                    let (content, sig, def_start, def_end) = if let Some(def) = def_node {
                        let def_text = def
                            .node
                            .utf8_text(source.as_bytes())
                            .unwrap_or("")
                            .to_string();
                        let first_line = def_text.lines().next().unwrap_or("").to_string();
                        (
                            def_text,
                            first_line,
                            def.node.start_position().row + 1,
                            def.node.end_position().row + 1,
                        )
                    } else {
                        (text.clone(), text.clone(), start_line, end_line)
                    };

                    // Build qualified name: file::parent::name
                    // For methods, look for impl.type in the same match
                    let parent_name = if prefix == "method" {
                        m.captures
                            .iter()
                            .find(|c| capture_names[c.index as usize] == "impl.type")
                            .and_then(|c| c.node.utf8_text(source.as_bytes()).ok())
                            .map(|s: &str| s.to_string())
                    } else {
                        None
                    };

                    let qualified_name = if let Some(ref parent) = parent_name {
                        format!("{}::{}::{}", file_path, parent, text)
                    } else {
                        format!("{}::{}", file_path, text)
                    };

                    let parent_qn = parent_name
                        .as_ref()
                        .map(|p| format!("{}::{}", file_path, p));

                    entities.push(CodeEntity {
                        qualified_name: qualified_name.clone(),
                        name: text.clone(),
                        entity_type,
                        language: lang,
                        source_file: file_path.to_string(),
                        content,
                        start_line: def_start,
                        end_line: def_end,
                        parent: parent_qn.clone(),
                        signature: sig,
                    });

                    // Add containment relationship
                    let container = parent_qn.unwrap_or_else(|| file_qn.clone());
                    relationships.push(CodeRelationship {
                        from_qualified_name: container,
                        to_qualified_name: qualified_name,
                        relationship_type: RelationshipType::Contains,
                    });
                }

                // Import paths
                "import.path" | "import.module" => {
                    let import_qn = format!("{}::import::{}", file_path, text);
                    entities.push(CodeEntity {
                        qualified_name: import_qn.clone(),
                        name: text.clone(),
                        entity_type: EntityType::Import,
                        language: lang,
                        source_file: file_path.to_string(),
                        content: text.clone(),
                        start_line,
                        end_line,
                        parent: Some(file_qn.clone()),
                        signature: text.clone(),
                    });
                    relationships.push(CodeRelationship {
                        from_qualified_name: file_qn.clone(),
                        to_qualified_name: import_qn.clone(),
                        relationship_type: RelationshipType::Contains,
                    });
                    relationships.push(CodeRelationship {
                        from_qualified_name: file_qn.clone(),
                        to_qualified_name: text.clone(),
                        relationship_type: RelationshipType::Imports,
                    });
                }

                // Function/method calls
                "call.name" => {
                    // Find the enclosing function to set as the caller
                    let caller = find_enclosing_function(&entities, file_path, start_line);
                    let callee = format!("{}::{}", file_path, text);
                    relationships.push(CodeRelationship {
                        from_qualified_name: caller,
                        to_qualified_name: callee,
                        relationship_type: RelationshipType::Calls,
                    });
                }

                "call.method_name" => {
                    let caller = find_enclosing_function(&entities, file_path, start_line);
                    // Method calls - we don't know the type, use unresolved name
                    relationships.push(CodeRelationship {
                        from_qualified_name: caller,
                        to_qualified_name: format!("*::{}", text),
                        relationship_type: RelationshipType::Calls,
                    });
                }

                // Inheritance
                "inherits.name" => {
                    // Find the enclosing class/struct
                    let child = find_enclosing_type(&entities, file_path, start_line);
                    relationships.push(CodeRelationship {
                        from_qualified_name: child,
                        to_qualified_name: text.clone(),
                        relationship_type: RelationshipType::Inherits,
                    });
                }

                "implements.name" => {
                    let implementor = find_enclosing_type(&entities, file_path, start_line);
                    relationships.push(CodeRelationship {
                        from_qualified_name: implementor,
                        to_qualified_name: text.clone(),
                        relationship_type: RelationshipType::Implements,
                    });
                }

                _ => {}
            }
        }
    }

    Ok(ParseResult {
        entities,
        relationships,
    })
}

/// Find the enclosing function/method for a given line number
fn find_enclosing_function(entities: &[CodeEntity], file_path: &str, line: usize) -> String {
    entities
        .iter()
        .filter(|e| {
            (e.entity_type == EntityType::Function || e.entity_type == EntityType::Method)
                && e.source_file == file_path
                && e.start_line <= line
                && e.end_line >= line
        })
        .last()
        .map(|e| e.qualified_name.clone())
        .unwrap_or_else(|| file_path.to_string())
}

/// Find the enclosing class/struct/trait for a given line number
fn find_enclosing_type(entities: &[CodeEntity], file_path: &str, line: usize) -> String {
    entities
        .iter()
        .filter(|e| {
            matches!(
                e.entity_type,
                EntityType::Class | EntityType::Struct | EntityType::Trait | EntityType::Enum
            ) && e.source_file == file_path
                && e.start_line <= line
                && e.end_line >= line
        })
        .last()
        .map(|e| e.qualified_name.clone())
        .unwrap_or_else(|| file_path.to_string())
}
