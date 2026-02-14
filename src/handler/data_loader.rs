use extractous::Extractor;
use rig::providers::ollama::Client;
use rig::client::{Nothing, EmbeddingsClient, ProviderClient};
use rig::embeddings::{EmbeddingsBuilder, Embedding};
use rig::OneOrMany;

const EMBEDDING_MODEL: &str = "qwen3-embedding:0.6b";
const EMBEDDING_NDIMS: usize = 512;
const CHUNK_SIZE: usize = 1000;  // Characters per chunk

/// Embedding result: document text paired with its embedding(s)
pub type EmbeddingResult = Vec<(String, OneOrMany<Embedding>)>;

/// Load text from a file
pub fn load_data(file_path: &str) -> anyhow::Result<String> {
    let extractor = Extractor::new();
    let (text, metadata) = extractor.extract_file_to_string(file_path)?;
    println!("Loaded file with metadata: {:?}", metadata);
    Ok(text)
}

/// Split text into chunks for embedding
fn chunk_text(text: &str, chunk_size: usize) -> Vec<String> {
    text.chars()
        .collect::<Vec<_>>()
        .chunks(chunk_size)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

/// Generate embeddings for text chunks
pub async fn embed_texts(texts: Vec<String>) -> anyhow::Result<EmbeddingResult> {
    let client = Client::from_val(Nothing);
    let embedder = client.embedding_model_with_ndims(EMBEDDING_MODEL, EMBEDDING_NDIMS);
    let embeddings = EmbeddingsBuilder::new(embedder)
        .documents(texts)?
        .build()
        .await?;
    Ok(embeddings)
}

/// Load a file and generate embeddings for its content
pub async fn load_and_embed(file_path: &str) -> anyhow::Result<EmbeddingResult> {
    // Load the file
    let text = load_data(file_path)?;
    println!("Loaded {} characters", text.len());

    // Chunk the text
    let chunks = chunk_text(&text, CHUNK_SIZE);
    println!("Split into {} chunks", chunks.len());

    // Generate embeddings
    let embeddings = embed_texts(chunks).await?;
    println!("Generated {} embeddings", embeddings.len());

    Ok(embeddings)
}