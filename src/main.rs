use cli_chat::models::chat_ollama::Message as OllamaMessage;
use cli_chat::services::chat::loop_chat;

#[tokio::main]
async fn main() {
    println!("Hi. How may I help you today?");

    // let mut message: String = String::from("What is the answer to life, universe and everything?");
    let history: Vec<OllamaMessage> = Vec::new();
    let system_prompt = String::from("You are a helpful assistant, who keeps the answers crisp and precise");
    loop_chat(&history, system_prompt.clone(), "Hi. How may I help you today?").await;
}

