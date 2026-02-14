use adk_model::ollama::{OllamaModel, OllamaConfig};
use adk_agent::LlmAgentBuilder;
use adk_rust::prelude::*;
use adk_rust::session::{CreateRequest, SessionService};
use cli_chat::tools::base_tool::BaseTool;
use cli_chat::tools::tool_structs::DocumentParser;
use cli_chat::models::chat_ollama::Message as OllamaMessage;
use cli_chat::services::chat::loop_chat;
use std::sync::Arc;

use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[arg(short, long)]
    operation: String,
    #[arg()]
    path: Option<std::path::PathBuf>,
}


const OLLAMA_HOST: &str = "http://0.0.0.0:11434";
const MODEL: &str = "llama3.2";

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    let Cli { operation, path } = Cli::parse();
    println!("operation: {}", operation);

    match operation.as_str() {
        "simple" => simple_chat_operation().await,
        "chat" => chat_operation().await,
        "loader" => {
            let file_path = path.ok_or_else(|| anyhow::anyhow!("`path` is required when operation is `loader`"))?;
            loader_operation(file_path).await
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
    use adk_rust::futures::StreamExt;

    let config: OllamaConfig = OllamaConfig::with_host(OLLAMA_HOST, MODEL);
    let model = OllamaModel::new(config)?;

    let agent = LlmAgentBuilder::new("local_assistant")
        .instruction("You are a helpful assistant running locally.")
        .model(
            Arc::new(model)
        )
        // .tool(Arc::new((DocumentParser {}).get_tool().unwrap()))
        .build()?;

    // Create session service and runner
    let session_service = Arc::new(InMemorySessionService::new());
    let runner = Runner::new(RunnerConfig {
        app_name: "cli_chat".to_string(),
        agent: Arc::new(agent),
        session_service: session_service.clone(),
        artifact_service: None,
        memory_service: None,
        run_config: None,
    })?;

    let user_id = "user".to_string();
    let session_id = "session_1".to_string();

    // Create session before starting the chat
    session_service.create(CreateRequest {
        app_name: "cli_chat".to_string(),
        user_id: user_id.clone(),
        session_id: Some(session_id.clone()),
        state: std::collections::HashMap::new(),
    }).await?;

    println!("Chat started. Type 'exit' or 'quit' to end the conversation.\n");

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

        // Create content from user input
        let content = Content::new("user").with_text(input);

        // Run agent
        match runner.run(user_id.clone(), session_id.clone(), content).await {
            Ok(mut events) => {
                print!("\nAssistant: ");
                io::stdout().flush()?;

                while let Some(event) = events.next().await {
                    match event {
                        Ok(evt) => {
                            if let Some(content) = evt.content() {
                                for part in &content.parts {
                                    if let Part::Text { text } = part {
                                        print!("{}", text);
                                        io::stdout().flush()?;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("\nStream error: {}", e);
                            break;
                        }
                    }
                }
                println!("\n");
            }
            Err(e) => {
                eprintln!("Error: {}\n", e);
            }
        }
    }

    Ok(())
}

async fn loader_operation(file_path: std::path::PathBuf) -> anyhow::Result<()> {
    println!("{:?}", file_path);
    Ok(())
}