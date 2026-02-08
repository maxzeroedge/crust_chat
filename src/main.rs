// use cli_chat::models::chat_ollama::Message as OllamaMessage;
// use cli_chat::services::chat::loop_chat;

// #[tokio::main]
// async fn main() {
//     simple_logger::init_with_level(log::Level::Info).unwrap();
//     log::info!("Hi. How may I help you today?");

//     // let mut message: String = String::from("What is the answer to life, universe and everything?");
//     let history: Vec<OllamaMessage> = Vec::new();
//     let system_prompt = String::from("You are a helpful assistant, who keeps the answers crisp and precise");
//     loop_chat(&history, system_prompt.clone(), "Hi. How may I help you today?").await;
// }

use adk_model::ollama::{OllamaModel, OllamaConfig};
use adk_agent::LlmAgentBuilder;
use adk_rust::Launcher;
use cli_chat::tools::base_tool::BaseTool;
use cli_chat::tools::tool_structs::DocumentParser;
use std::sync::Arc;

const OLLAMA_HOST: &str = "http://0.0.0.0:11434";
const MODEL: &str = "llama3.2";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config: OllamaConfig = OllamaConfig::with_host(OLLAMA_HOST, MODEL);
    let model = OllamaModel::new(config)?;

    let agent = LlmAgentBuilder::new("local_assistant")
        .instruction("You are a helpful assistant running locally.")
        .model(
            Arc::new(model)
        )
        .tool(Arc::new((DocumentParser {}).get_tool().unwrap()))
        .build()?;
    Launcher::new(Arc::new(agent)).run().await?;

    Ok(())
}
