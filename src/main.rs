use cli_chat::models::chat_ollama::Message as OllamaMessage;
use cli_chat::services::chat::loop_chat;
use cli_chat::handler::data_loader::load_embed_and_store;
use cli_chat::services::rag::generate_rag_response;
use cli_chat::db::vector_store::{init_pool, init_schema};

use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[arg(short, long)]
    operation: String,
    #[arg(short, long)]
    path: Option<std::path::PathBuf>,
    #[arg(short, long, default_value_t = false)]
    force: bool,
}


#[tokio::main]
async fn main() -> anyhow::Result<()> {

    let Cli { operation, path, force } = Cli::parse();
    println!("operation: {}", operation);

    match operation.as_str() {
        "simple" => simple_chat_operation().await,
        "chat" => chat_operation().await,
        "loader" => {
            let file_path = path.ok_or_else(|| anyhow::anyhow!("`path` is required when operation is `loader`"))?;
            loader_operation(file_path, force).await
        }
        _ => {
            println!("Use simple, chat, or loader as operation");
            Ok(())
        },
    }

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

        match generate_rag_response(&pool, input).await {
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

async fn loader_operation(file_path: std::path::PathBuf, force: bool) -> anyhow::Result<()> {
    let path_str = file_path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid path"))?;
    load_embed_and_store(path_str, force).await?;
    Ok(())
}