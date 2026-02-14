use extractous::Extractor;
use rig::providers::ollama::Client;
use rig::client::{Nothing, EmbeddingsClient};
use rig::embeddings::{EmbeddingsBuilder, Embedding};
use rig::OneOrMany;
use std::env;
use std::time::Instant;
use std::io::{self, Write};

const EMBEDDING_MODEL: &str = "qwen3-embedding:0.6b";
const EMBEDDING_NDIMS: usize = 512;
const CHUNK_SIZE: usize = 1000;  // Characters per chunk
const BATCH_SIZE: usize = 10;   // Chunks per embedding batch

/// Get embedding Ollama client with configured host
fn get_embedding_client() -> Client {
    dotenvy::dotenv_override().ok();

    let host = env::var("EMBEDDING_MODEL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("EMBEDDING_MODEL_PORT").unwrap_or_else(|_| "11434".to_string());
    let base_url = format!("http://{}:{}", host, port);

    Client::builder()
        .api_key(Nothing)
        .base_url(&base_url)
        .build()
        .expect("Failed to create embedding client")
}

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
    let client = get_embedding_client();
    let embedder = client.embedding_model_with_ndims(EMBEDDING_MODEL, EMBEDDING_NDIMS);
    let embeddings = EmbeddingsBuilder::new(embedder)
        .documents(texts)?
        .build()
        .await?;
    // Truncate to EMBEDDING_NDIMS (rig's Ollama provider doesn't send ndims to the API)
    let truncated = embeddings
        .into_iter()
        .map(|(text, emb)| {
            let e = emb.first();
            let truncated_vec: Vec<f64> = e.vec.into_iter().take(EMBEDDING_NDIMS).collect();
            let doc = e.document.clone();
            (text, OneOrMany::one(Embedding { document: doc, vec: truncated_vec }))
        })
        .collect();
    Ok(truncated)
}

/// Generate embedding for a single query
pub async fn embed_query(query: &str) -> anyhow::Result<Vec<f32>> {
    let client = get_embedding_client();
    let embedder = client.embedding_model_with_ndims(EMBEDDING_MODEL, EMBEDDING_NDIMS);
    let embeddings = EmbeddingsBuilder::new(embedder)
        .document(query.to_string())?
        .build()
        .await?;

    let (_, embedding) = embeddings.into_iter().next()
        .ok_or_else(|| anyhow::anyhow!("No embedding generated"))?;

    let vec: Vec<f32> = embedding.first().vec.into_iter().take(EMBEDDING_NDIMS).map(|x| x as f32).collect();
    Ok(vec)
}

/// Load a file and generate embeddings for its content
pub async fn load_and_embed(file_path: &str) -> anyhow::Result<EmbeddingResult> {
    // Load the file
    print!("Loading file...              ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let text = load_data(file_path)?;
    println!("{:.2}s ({} chars)", start.elapsed().as_secs_f64(), text.len());

    // Chunk the text
    print!("Chunking text...             ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let chunks = chunk_text(&text, CHUNK_SIZE);
    let total_chunks = chunks.len();
    println!("{:.2}s ({} chunks)", start.elapsed().as_secs_f64(), total_chunks);

    // Generate embeddings in batches with progress
    let mut all_embeddings: EmbeddingResult = Vec::new();
    let total_batches = (total_chunks + BATCH_SIZE - 1) / BATCH_SIZE;
    let mut embed_total_time = 0.0;

    for (i, batch) in chunks.chunks(BATCH_SIZE).enumerate() {
        let processed = ((i + 1) * BATCH_SIZE).min(total_chunks);
        print!("\rEmbedding batch {}/{}...       ", i + 1, total_batches);
        io::stdout().flush().ok();

        let start = Instant::now();
        let batch_embeddings = embed_texts(batch.to_vec()).await?;
        let batch_time = start.elapsed().as_secs_f64();
        embed_total_time += batch_time;

        print!("\rEmbedding batch {}/{}...       {:.2}s ({} chunks)", i + 1, total_batches, batch_time, processed);
        println!();

        all_embeddings.extend(batch_embeddings);
    }
    println!("Embedding total:             {:.2}s ({} embeddings)", embed_total_time, all_embeddings.len());

    Ok(all_embeddings)
}

/// Load a file, generate embeddings, and store them in PostgreSQL
/// If `force` is false and the file was already processed, skips loading.
pub async fn load_embed_and_store(file_path: &str, force: bool) -> anyhow::Result<usize> {
    use crate::db::vector_store::{init_pool, init_schema, store_embeddings, file_exists};

    let total_start = Instant::now();

    // Connect to database
    print!("Connecting to PostgreSQL...  ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let pool = init_pool().await?;
    println!("{:.2}s", start.elapsed().as_secs_f64());

    // Initialize schema
    print!("Initializing schema...       ");
    io::stdout().flush().ok();
    let start = Instant::now();
    init_schema(&pool).await?;
    println!("{:.2}s", start.elapsed().as_secs_f64());

    // Check if file already exists
    if !force && file_exists(&pool, file_path).await? {
        println!("File '{}' already embedded, skipping (use --force to reload)", file_path);
        return Ok(0);
    }

    println!("Processing: {}", file_path);
    println!("{}", "-".repeat(45));

    // Load and embed the file
    let embeddings = load_and_embed(file_path).await?;

    // Store embeddings in database
    print!("Storing to database...       ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let count = store_embeddings(&pool, embeddings, file_path).await?;
    println!("{:.2}s ({} rows)", start.elapsed().as_secs_f64(), count);

    println!("{}", "-".repeat(45));
    println!("Total:                       {:.2}s", total_start.elapsed().as_secs_f64());

    Ok(count)
}