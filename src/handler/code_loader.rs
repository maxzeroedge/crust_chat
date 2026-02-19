use std::io::{self, Write};
use std::time::Instant;

use crate::db::graph_store::{
    delete_file_entities, init_graph, init_graph_schema, store_entities, store_relationships,
};
use crate::db::vector_store::{
    delete_file_embeddings, file_exists, init_pool, init_schema, store_code_embeddings,
};
use crate::handler::data_loader::embed_texts;
use crate::parser::entity::EntityType;
use crate::parser::tree_sitter_parser::parse_file;
use crate::parser::CodeLanguage;

const BATCH_SIZE: usize = 10;

pub async fn load_code_and_store(
    file_path: &str,
    lang: CodeLanguage,
    force: bool,
) -> anyhow::Result<usize> {
    let total_start = Instant::now();

    println!("Processing code file: {} [{}]", file_path, lang);
    println!("{}", "-".repeat(50));

    // Connect to PostgreSQL
    print!("Connecting to PostgreSQL...  ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let pool = init_pool().await?;
    init_schema(&pool).await?;
    println!("{:.2}s", start.elapsed().as_secs_f64());

    // Connect to Neo4j
    print!("Connecting to Neo4j...       ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let graph = init_graph().await?;
    init_graph_schema(&graph).await?;
    println!("{:.2}s", start.elapsed().as_secs_f64());

    // Check if already processed
    if !force && file_exists(&pool, file_path).await? {
        println!(
            "File '{}' already processed, skipping (use --force to reload)",
            file_path
        );
        return Ok(0);
    }

    // Read source code
    print!("Reading source file...       ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let source = std::fs::read_to_string(file_path)?;
    println!(
        "{:.2}s ({} chars)",
        start.elapsed().as_secs_f64(),
        source.len()
    );

    // Parse with tree-sitter
    print!("Parsing AST...               ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let parse_result = parse_file(file_path, &source, lang)?;
    println!(
        "{:.2}s ({} entities, {} relationships)",
        start.elapsed().as_secs_f64(),
        parse_result.entities.len(),
        parse_result.relationships.len()
    );

    // Delete old data if reloading
    if force {
        delete_file_embeddings(&pool, file_path).await?;
        delete_file_entities(&graph, file_path).await?;
    }

    // Collect file-level imports to prepend to each entity's content
    let imports_block: String = parse_result
        .entities
        .iter()
        .filter(|e| e.entity_type == EntityType::Import)
        .map(|e| e.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    let context_header = if imports_block.is_empty() {
        format!("// From {}\n", file_path)
    } else {
        format!("// From {}\n{}\n\n", file_path, imports_block)
    };

    // Embed entities (skip File, Import, and short/empty content)
    const MIN_CONTENT_LEN: usize = 50;
    let embeddable: Vec<_> = parse_result
        .entities
        .iter()
        .filter(|e| {
            e.entity_type != EntityType::File
                && e.entity_type != EntityType::Import
                && e.content.len() >= MIN_CONTENT_LEN
        })
        .collect();

    let mut total_stored = 0;
    let total_batches = (embeddable.len() + BATCH_SIZE - 1) / BATCH_SIZE;

    for (i, batch) in embeddable.chunks(BATCH_SIZE).enumerate() {
        print!(
            "\rEmbedding & storing batch {}/{}...  ",
            i + 1,
            total_batches
        );
        io::stdout().flush().ok();

        let start = Instant::now();
        let texts: Vec<String> = batch
            .iter()
            .map(|e| format!("{}{}", context_header, e.content))
            .collect();
        let embeddings = embed_texts(texts).await?;

        // Store each embedding with its entity metadata
        for (j, (content, emb_data)) in embeddings.into_iter().enumerate() {
            let entity = &batch[j];
            store_code_embeddings(
                &pool,
                vec![(content, emb_data)],
                file_path,
                &entity.entity_type.to_string(),
                &entity.qualified_name,
                &lang.to_string(),
            )
            .await?;
            total_stored += 1;
        }

        println!(
            "\rEmbedding & storing batch {}/{}...  {:.2}s",
            i + 1,
            total_batches,
            start.elapsed().as_secs_f64()
        );
    }

    // Store graph
    print!("Storing graph nodes...       ");
    io::stdout().flush().ok();
    let start = Instant::now();
    store_entities(&graph, &parse_result.entities).await?;
    println!("{:.2}s", start.elapsed().as_secs_f64());

    print!("Storing graph edges...       ");
    io::stdout().flush().ok();
    let start = Instant::now();
    store_relationships(&graph, &parse_result.relationships).await?;
    println!("{:.2}s", start.elapsed().as_secs_f64());

    println!("{}", "-".repeat(50));
    println!(
        "Total: {:.2}s ({} embeddings, {} nodes, {} edges)",
        total_start.elapsed().as_secs_f64(),
        total_stored,
        parse_result.entities.len(),
        parse_result.relationships.len()
    );

    Ok(total_stored)
}
