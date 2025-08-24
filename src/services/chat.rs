use chrono::Utc;
use std::collections::{HashMap};
use std::vec::Vec;
use std::io;
use std::error::Error;
use reqwest::header::HeaderMap;
use reqwest::{Client, StatusCode};
use std::path::Path;
use std::fs::File;
use std::io::Read;
use base64::{Engine as _, engine::{general_purpose}};
use mime_guess::from_path;


use crate::models::chat_ollama::{
    EncodedFile, Message as OllamaMessage, MessageRole as OllamaMessageRole
};

use crate::tools::base_tool::BaseToolCall;

// const MODEL_NAME: &str = "gemma3:4b-it-q4_K_M"; // Doesn't support tools
// const MODEL_NAME: &str = "qwen3:1.7b";
// const MODEL_NAME: &str = "llama3.2:3b-instruct-q5_1";
const MODEL_NAME: &str = "gpt-oss:latest";
const IMAGE_MODEL_NAME: &str = "llama3.2-vision:11b-instruct-q4_K_M";
const OLLAMA_SERVER_URL: &str = "http://localhost:11434";

pub async fn loop_chat(history: &Vec<OllamaMessage>, system_prompt: String, start_message: &str) {
    let mut message: String = String::new();
    io::stdin()
        .read_line(&mut message)
        .expect(start_message);

    if message.eq("/bye") || message.eq("/exit") || message.eq("/quit") {
        std::process::exit(0);
    }

    let start_time = Utc::now().time();
    let new_history = chat_with_ollama(OllamaMessage {
        role: OllamaMessageRole::USER,
        content: message,
        files: None
    }, history.clone(), system_prompt.clone()).await.unwrap();
    
    let end_time = Utc::now().time();
    let next_start_message: String = format!(
        "{}\n====> In {} ms. <====\n",
        new_history.last().unwrap().content,
        (end_time - start_time).num_milliseconds()
    );
    log::info!("{}", next_start_message);

    Box::pin(async {
        loop_chat(&new_history, system_prompt, next_start_message.as_str()).await
    }).await;
}

fn encode_file(file_path: &str) -> std::io::Result<String> {
    // Guess MIME type from file path
    let path = Path::new(file_path);
    let mime_type = from_path(path)
        .first_or_octet_stream()
        .to_string();

    // Read file content
    let mut file = File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Encode to base64
    let encoded_str = general_purpose::STANDARD.encode(&buffer);

    // Return data URL
    Ok(format!("data:{};base64,{}", mime_type, encoded_str))
}

async fn chat_with_ollama(message: OllamaMessage, history: Vec<OllamaMessage>, system_prompt: String) -> Result<Vec<OllamaMessage>, Box<dyn Error>> {
    // log::info!("{}", message["content"]);
    let history_message = OllamaMessage {
        role: OllamaMessageRole::USER,
        content: message.content,
        files: None
    };
    if message.files.is_some() {
        let mut encoded_files: HashMap<String, EncodedFile> = HashMap::new();
        // for file in message.files.unwrap().iter() {
        for file in message.files.as_ref().unwrap().iter() {
            let file_name = Path::file_name(Path::new(file.file_name.as_str()))
                .and_then(|name| name.to_str())
                .map(String::from)
                .unwrap();
            encoded_files.insert(file_name.clone(), EncodedFile {
                file_name: file_name.clone(),
                data: encode_file(file.file_name.as_str()).unwrap()
            });
        }
        
        // TODO:
        // history_message.files = Some(encoded_files);
    }

    let mut mut_history = history.clone();
    mut_history.push(history_message);
    let mut images: Vec<String> = Vec::new();

    // Image Search
    for h in &history {
        if h.files.is_some() {
            for data in h.files.as_ref().unwrap().iter() {
                images.push(data.data.to_string());
            }
        }
    }

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse().unwrap());

    let mut model_name = MODEL_NAME;

    if !images.is_empty() {
        model_name = IMAGE_MODEL_NAME;
    }

    let tool_manager = BaseToolCall::new();

    let data = serde_json::json!({
            "model": model_name,
            "messages": mut_history,
            "prefix": system_prompt,
            "stream": false,
            "images": images,
            "suffix": " Keep answers precise",
            "tools": tool_manager.get_available_tool_definitions(),
    });
    // data.insert("suffix", " Respond as JSON with only one key: answer. Never include extra keys such as description or anything");
    
    log::info!("Checking....");
    
    let url = String::from(OLLAMA_SERVER_URL).to_owned() + "/api/chat";
    let response = Client::new().post(url)
        .json(&data)
        .headers(headers.clone())
        .send()
        .await?;

    match response.status() {
        StatusCode::INTERNAL_SERVER_ERROR => {
            log::error!("{:?}", response.text().await.unwrap());
            return Err("Server Error".into());
        }
        StatusCode::BAD_REQUEST => {
            log::error!("{:?}", response.text().await.unwrap());
            return Err("Bad Request".into());
        }
        _ => {
            match response.json::<serde_json::Value>().await {
                Ok(response_json) => {
                    log::info!("{:?}", response_json["message"]);
                    let message = response_json["message"].as_object().unwrap();
                    let mut content = response_json["message"].get("content").unwrap().to_string();
                    if message.contains_key("tool_calls") {
                        content = tool_manager.run_respective_tools(message["tool_calls"].as_array().unwrap().to_vec()).await;
                    }
                    mut_history.push(OllamaMessage {
                        role: OllamaMessageRole::ASSISTANT,
                        content: content,
                        files: None 
                    });
                    return Ok(mut_history);
                }
                Err(e) => {
                    log::error!("Got an error {:?}", e.status());
                    return Err(e.into());
                }
            }
        }
    }
}
