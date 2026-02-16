use extractous::Extractor;
use std::env;
use std::io::{self, Write};
use std::time::Instant;

use crate::db::vector_store::{
    delete_file_embeddings, file_exists, init_pool, init_schema, store_code_embeddings,
};
use crate::handler::data_loader::{chunk_text, embed_texts};

const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "tiff", "tif", "bmp",
];

const VISION_PROMPT: &str = "Describe this image in detail. Include any text visible in the image, \
    the layout, diagrams, charts, tables, or any other visual elements. \
    Be thorough and precise.";

/// Check if a file is an image by extension
pub fn detect_image(file_path: &str) -> bool {
    std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Send image bytes to Ollama vision model and get a description
pub async fn describe_image_bytes(image_bytes: &[u8], prompt: &str) -> anyhow::Result<String> {
    dotenvy::dotenv_override().ok();

    let host = env::var("VISION_MODEL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("VISION_MODEL_PORT").unwrap_or_else(|_| "11434".to_string());
    let model = env::var("VISION_MODEL")?;

    let encoded = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        image_bytes,
    );

    let url = format!("http://{}:{}/api/chat", host, port);
    let body = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": prompt,
            "images": [encoded]
        }],
        "stream": false
    });

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await?;

    let json: serde_json::Value = response.json().await?;
    let description = json["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(description)
}

/// Send image file to Ollama vision model and get a description
async fn describe_image(file_path: &str) -> anyhow::Result<String> {
    let image_bytes = std::fs::read(file_path)?;
    describe_image_bytes(&image_bytes, VISION_PROMPT).await
}

/// Extract text from image using extractous OCR
fn ocr_image(file_path: &str) -> anyhow::Result<String> {
    let extractor = Extractor::new();
    match extractor.extract_file_to_string(file_path) {
        Ok((text, _metadata)) => Ok(text.trim().to_string()),
        Err(_) => Ok(String::new()),
    }
}

/// Load an image file, run OCR + vision model, embed and store
pub async fn load_image_and_store(file_path: &str, force: bool) -> anyhow::Result<usize> {
    let total_start = Instant::now();

    println!("Processing image: {}", file_path);
    println!("{}", "-".repeat(50));

    // Connect to PostgreSQL
    print!("Connecting to PostgreSQL...  ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let pool = init_pool().await?;
    init_schema(&pool).await?;
    println!("{:.2}s", start.elapsed().as_secs_f64());

    // Check if already processed
    if !force && file_exists(&pool, file_path).await? {
        println!(
            "File '{}' already processed, skipping (use --force to reload)",
            file_path
        );
        return Ok(0);
    }

    // Delete old data if reloading
    if force {
        delete_file_embeddings(&pool, file_path).await?;
    }

    // Run OCR and vision model in parallel
    print!("Running OCR...               ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let ocr_text = ocr_image(file_path)?;
    println!(
        "{:.2}s ({} chars)",
        start.elapsed().as_secs_f64(),
        ocr_text.len()
    );

    print!("Getting vision description.. ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let description = describe_image(file_path).await?;
    println!(
        "{:.2}s ({} chars)",
        start.elapsed().as_secs_f64(),
        description.len()
    );

    // Combine OCR text and vision description
    let mut combined = String::new();
    if !ocr_text.is_empty() {
        combined.push_str("[OCR Text]\n");
        combined.push_str(&ocr_text);
        combined.push_str("\n\n");
    }
    if !description.is_empty() {
        combined.push_str("[Image Description]\n");
        combined.push_str(&description);
    }

    if combined.trim().is_empty() {
        println!("No text or description extracted from image, skipping");
        return Ok(0);
    }

    // Chunk the combined text
    print!("Chunking text...             ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let chunks = chunk_text(&combined, 1000);
    println!(
        "{:.2}s ({} chunks)",
        start.elapsed().as_secs_f64(),
        chunks.len()
    );

    // Embed and store
    print!("Embedding & storing...       ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let embeddings = embed_texts(chunks).await?;
    let count = store_code_embeddings(&pool, embeddings, file_path, "image", file_path, "")
        .await?;
    println!("{:.2}s ({} rows)", start.elapsed().as_secs_f64(), count);

    println!("{}", "-".repeat(50));
    println!(
        "Total: {:.2}s",
        total_start.elapsed().as_secs_f64()
    );

    Ok(count)
}
