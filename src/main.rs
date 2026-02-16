use cli_chat::models::chat_ollama::Message as OllamaMessage;
use cli_chat::services::chat::loop_chat;
use cli_chat::handler::data_loader::load_embed_and_store;
use cli_chat::services::rag::{generate_rag_response, retrieve_context};
use rig::completion::Message;
use cli_chat::db::vector_store::{init_pool, init_schema};
use cli_chat::db::graph_store::init_graph;

use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[arg(short, long)]
    operation: String,
    #[arg(short, long)]
    path: Option<std::path::PathBuf>,
    #[arg(short, long, default_value_t = false)]
    force: bool,
    #[arg(short, long)]
    query: Option<String>,
}


#[tokio::main]
async fn main() -> anyhow::Result<()> {

    let Cli { operation, path, force, query } = Cli::parse();
    println!("operation: {}", operation);

    // Verify database connections on startup
    check_connections().await?;

    match operation.as_str() {
        "simple" => simple_chat_operation().await,
        "chat" => chat_operation().await,
        "loader" => {
            let file_path = path.ok_or_else(|| anyhow::anyhow!("`path` is required when operation is `loader`"))?;
            loader_operation(file_path, force).await
        }
        "search" => {
            let q = query.ok_or_else(|| anyhow::anyhow!("`query` (-q) is required when operation is `search`"))?;
            search_operation(&q).await
        }
        _ => {
            println!("Use simple, chat, loader, or search as operation");
            Ok(())
        },
    }

}

async fn check_connections() -> anyhow::Result<()> {
    print!("Checking PostgreSQL...  ");
    let pool = init_pool().await?;
    sqlx::query("SELECT 1").execute(&pool).await?;
    println!("OK");

    print!("Checking Neo4j...       ");
    let graph = init_graph().await?;
    graph.run(neo4rs::query("RETURN 1")).await?;
    println!("OK");

    println!();
    Ok(())
}

async fn simple_chat_operation() -> anyhow::Result<()> {
    simple_logger::init_with_level(log::Level::Info).unwrap();
    log::info!("Hi. How may I help you today?");

    // let mut message: String = String::from("What is the answer to life, universe and everything?");
    let history: Vec<OllamaMessage> = Vec::new();
    let system_prompt = String::from("You are a helpful assistant, who keeps the answers crisp and precise");
    loop_chat(&history, system_prompt.clone(), "Hi. How may I help you today?").await;

    Ok(())

}

async fn chat_operation() -> anyhow::Result<()> {
    use std::io::{self, Write};

    // Connect to knowledge base
    println!("Connecting to knowledge base...");
    let pool = init_pool().await?;
    init_schema(&pool).await?;

    println!("RAG Chat started. Type 'exit' or 'quit' to end.\n");

    let mut chat_history: Vec<Message> = Vec::new();

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input == "exit" || input == "quit" {
            println!("Goodbye!");
            break;
        }

        println!("\nSearching knowledge base...");

        match generate_rag_response(&pool, input, &mut chat_history).await {
            Ok(response) => {
                println!("\nAssistant: {}\n", response);
            }
            Err(e) => {
                eprintln!("Error: {}\n", e);
            }
        }
    }

    Ok(())
}

async fn search_operation(query: &str) -> anyhow::Result<()> {
    let pool = init_pool().await?;
    init_schema(&pool).await?;

    println!("Query: {}\n", query);

    let (results, embed_time, search_time) = retrieve_context(&pool, query).await?;

    println!("Found {} results (embed: {:.2}s, search: {:.2}s)\n",
        results.len(), embed_time, search_time);

    if results.is_empty() {
        println!("No results above similarity threshold.");
        return Ok(());
    }

    for (i, r) in results.iter().enumerate() {
        let etype = r.entity_type.as_deref().unwrap_or("-");
        let ename = r.entity_name.as_deref().unwrap_or("-");
        let source = std::path::Path::new(&r.source_file)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| r.source_file.clone());

        println!("[{}] similarity: {:.4} | type: {} | name: {} | source: {}",
            i + 1, r.similarity, etype, ename, source);

        // Show truncated content preview
        let preview: String = r.content.chars().take(200).collect();
        let ellipsis = if r.content.len() > 200 { "..." } else { "" };
        println!("    {}{}\n", preview.replace('\n', "\n    "), ellipsis);
    }

    Ok(())
}

async fn loader_operation(path: std::path::PathBuf, force: bool) -> anyhow::Result<()> {
    if path.is_file() {
        let path_str = path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
        load_embed_and_store(path_str, force).await?;
    } else if path.is_dir() {
        let files = collect_files(&path)?;
        println!("Found {} files in {}\n", files.len(), path.display());
        for (i, file) in files.iter().enumerate() {
            let file_str = file.to_string_lossy();
            println!("[{}/{}] {}", i + 1, files.len(), file_str);
            load_embed_and_store(&file_str, force).await?;
            println!();
        }
        println!("Done: {} files processed successfully", files.len());
    } else {
        anyhow::bail!("Path does not exist: {}", path.display());
    }
    Ok(())
}

/// Recursively collect all processable files from a directory
fn collect_files(dir: &std::path::Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    use cli_chat::parser::detect_language;
    use cli_chat::handler::image_loader::detect_image;

    // Extensions supported by extractous (document files)
    const DOC_EXTENSIONS: &[&str] = &[
        "pdf", "doc", "docx", "ppt", "pptx", "xls", "xlsx",
        "txt", "md", "csv", "json", "xml", "html", "htm", "rtf", "odt",
    ];

    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&current)?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                // Skip hidden dirs and common non-content dirs
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if !name.starts_with('.')
                    && name != "node_modules"
                    && name != "target"
                    && name != "__pycache__"
                    && name != "venv"
                    && name != ".git"
                {
                    stack.push(path);
                }
            } else if path.is_file() {
                let path_str = path.to_string_lossy();
                let is_code = detect_language(&path_str).is_some();
                let is_image = detect_image(&path_str);
                let is_doc = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| DOC_EXTENSIONS.contains(&e.to_lowercase().as_str()))
                    .unwrap_or(false);
                if is_code || is_doc || is_image {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    Ok(files)
}