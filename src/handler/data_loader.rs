use extractous::Extractor;
use rig::providers::ollama::Client;

pub fn load_data(file_path: &str) {
    let mut extractor = Extractor::new().set_extract_string_max_length(512);
    let (text, metadata) = extractor.extract_file_to_string(file_path).unwrap();
    println!("{:?}", text);
    println!("{:?}", metadata);
}

pub async fn embed_data(text_data: Vec<&str>) {
    let client = Client::new(Nothing).unwrap();
    let embedder = client.embedding_model(
        "qwen3-embedding:0.6b", 512
    );
    embedder.embed_texts(text_data).await?
}