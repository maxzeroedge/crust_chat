use cli_chat::models::chat_ollama::Message as OllamaMessage;
use cli_chat::services::chat::loop_chat;
use cli_chat::handler::data_loader::load_embed_and_store;
use cli_chat::services::rag::{generate_rag_response, retrieve_context};
use cli_chat::services::code_agent;
use cli_chat::services::project_agent;
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // Set up Ctrl+C handler: first press cancels current operation, second press quits
    let ctrlc_pressed = Arc::new(AtomicBool::new(false));
    let ctrlc_flag = ctrlc_pressed.clone();
    ctrlc::set_handler(move || {
        if ctrlc_flag.load(Ordering::SeqCst) {
            // Second Ctrl+C — exit immediately
            println!("\nForce quit.");
            std::process::exit(0);
        }
        ctrlc_flag.store(true, Ordering::SeqCst);
        eprintln!("\nInterrupted. Press Ctrl+C again to quit, or type a new query.");
    })?;

    // Connect to knowledge base
    println!("Connecting to knowledge base...");
    let pool = init_pool().await?;
    init_schema(&pool).await?;

    println!("RAG Chat started. Type 'exit' or 'quit' to end.");
    println!("  /load <path>    - load a file or directory into the knowledge base");
    println!("  /reload <path>  - force reload a file or directory (re-index)");
    println!("  /save <path>    - extract code from last response and save to file");
    println!("  /create [path]  - create a project from last response, verify it builds\n");

    let mut chat_history: Vec<Message> = Vec::new();
    let mut last_response: Option<String> = None;

    loop {
        // Reset Ctrl+C flag at each prompt
        ctrlc_pressed.store(false, Ordering::SeqCst);

        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                // Ctrl+C during read_line can cause an interrupted error
                if e.kind() == io::ErrorKind::Interrupted {
                    println!();
                    continue;
                }
                return Err(e.into());
            }
        }
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        if input == "exit" || input == "quit" {
            println!("Goodbye!");
            break;
        }

        // Handle /reload command (force reload)
        if input.starts_with("/reload") {
            let path = input.strip_prefix("/reload").unwrap().trim();
            if path.is_empty() {
                println!("Usage: /reload <file_or_directory_path>\n");
                continue;
            }
            let p = std::path::PathBuf::from(path);
            match loader_operation(p, true).await {
                Ok(_) => println!("Reload complete.\n"),
                Err(e) => eprintln!("Reload failed: {}\n", e),
            }
            continue;
        }

        // Handle /load command
        if input.starts_with("/load") {
            let path = input.strip_prefix("/load").unwrap().trim();
            if path.is_empty() {
                println!("Usage: /load <file_or_directory_path>\n");
                continue;
            }
            let p = std::path::PathBuf::from(path);
            match loader_operation(p, false).await {
                Ok(_) => println!("Loading complete.\n"),
                Err(e) => eprintln!("Loading failed: {}\n", e),
            }
            continue;
        }

        // Handle /save command
        if input.starts_with("/save") {
            let path = input.strip_prefix("/save").unwrap().trim();
            if path.is_empty() {
                println!("Usage: /save <file_path>\n");
                continue;
            }
            match &last_response {
                Some(resp) => {
                    match code_agent::extract_and_save(resp, path).await {
                        Ok(_) => {}
                        Err(e) => eprintln!("Code extraction failed: {}\n", e),
                    }
                }
                None => println!("No previous response to extract code from.\n"),
            }
            continue;
        }

        // Handle /create command
        if input.starts_with("/create") {
            let path = input.strip_prefix("/create").unwrap().trim();
            let output_dir = if path.is_empty() { None } else { Some(path) };
            match &last_response {
                Some(resp) => {
                    match project_agent::create_and_verify(resp, output_dir).await {
                        Ok(dir) => println!("Project ready at: {}\n", dir.display()),
                        Err(e) => eprintln!("Project creation failed: {}\n", e),
                    }
                }
                None => println!("No previous response to create project from.\n"),
            }
            continue;
        }

        println!("\nSearching knowledge base...");

        match generate_rag_response(&pool, input, &mut chat_history).await {
            Ok(rag) => {
                println!("\nAssistant: {}\n", rag.answer);
                last_response = Some(rag.answer.clone());

                if !rag.contexts.is_empty() {
                    println!("References:");
                    for (i, ctx) in rag.contexts.iter().enumerate() {
                        let source = std::path::Path::new(&ctx.source_file)
                            .file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_else(|| ctx.source_file.clone());
                        let etype = ctx.entity_type.as_deref().unwrap_or("-");
                        let ename = ctx.entity_name.as_deref().unwrap_or("-");
                        println!(
                            "  [{}] {} | type: {} | name: {} | similarity: {:.4}",
                            i + 1, source, etype, ename, ctx.similarity
                        );
                    }
                    println!();
                }
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