use extractous::{Extractor, PdfParserConfig, PdfOcrStrategy};
use rig::providers::ollama::Client;
use rig::client::{Nothing, EmbeddingsClient};
use rig::embeddings::{EmbeddingsBuilder, Embedding};
use rig::OneOrMany;
use std::env;
use std::path::Path;
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

/// Load text from a file. Tries OCR+text extraction first, falls back to no-OCR on failure.
pub fn load_data(file_path: &str) -> anyhow::Result<String> {
    let extractor_with_ocr = Extractor::new()
        .set_pdf_config(
            PdfParserConfig::new()
                .set_ocr_strategy(PdfOcrStrategy::OCR_AND_TEXT_EXTRACTION),
        );

    match extractor_with_ocr.extract_file_to_string(file_path) {
        Ok((text, metadata)) => {
            println!("Loaded file (with OCR) metadata: {:?}", metadata);
            Ok(text)
        }
        Err(e) => {
            println!("OCR extraction failed ({}), retrying without OCR...", e);
            let extractor_no_ocr = Extractor::new()
                .set_pdf_config(
                    PdfParserConfig::new()
                        .set_ocr_strategy(PdfOcrStrategy::NO_OCR),
                );
            let (text, metadata) = extractor_no_ocr.extract_file_to_string(file_path)?;
            println!("Loaded file (no OCR) metadata: {:?}", metadata);
            Ok(text)
        }
    }
}

/// Check if a file is a PDF
fn is_pdf(file_path: &str) -> bool {
    Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

const PDF_VISION_PROMPT: &str = "Extract all text from this page image. Include headings, paragraphs, \
    code snippets, table contents, figure captions, and any other visible text. \
    Preserve the reading order and structure. Output only the extracted text.";

/// Convert PDF pages to images using pdftoppm and run vision model OCR on each page
async fn pdf_vision_ocr(file_path: &str) -> anyhow::Result<String> {
    let tmp_dir = tempfile::tempdir()?;
    let tmp_prefix = tmp_dir.path().join("page");

    // Convert PDF to PNG images (one per page)
    print!("Converting PDF to images...  ");
    io::stdout().flush().ok();
    let start = Instant::now();

    let output = std::process::Command::new("pdftoppm")
        .arg("-png")
        .arg("-r")
        .arg("200") // 200 DPI - good balance of quality vs size
        .arg(file_path)
        .arg(tmp_prefix.to_str().unwrap())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("pdftoppm failed: {}", stderr);
    }

    // Collect page image files (sorted by name = page order)
    let mut page_files: Vec<_> = std::fs::read_dir(tmp_dir.path())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
        .collect();
    page_files.sort();

    println!(
        "{:.2}s ({} pages)",
        start.elapsed().as_secs_f64(),
        page_files.len()
    );

    if page_files.is_empty() {
        return Ok(String::new());
    }

    // Process each page with vision model
    let mut all_text = String::new();
    let total_pages = page_files.len();

    for (i, page_path) in page_files.iter().enumerate() {
        print!(
            "\rVision OCR page {}/{}...      ",
            i + 1,
            total_pages
        );
        io::stdout().flush().ok();
        let start = Instant::now();

        let image_bytes = std::fs::read(page_path)?;
        let description = crate::handler::image_loader::describe_image_bytes(
            &image_bytes,
            PDF_VISION_PROMPT,
        )
        .await?;

        println!(
            "\rVision OCR page {}/{}...      {:.2}s ({} chars)",
            i + 1,
            total_pages,
            start.elapsed().as_secs_f64(),
            description.len()
        );

        if !description.is_empty() {
            all_text.push_str(&format!("[Page {}]\n", i + 1));
            all_text.push_str(&description);
            all_text.push_str("\n\n");
        }
    }

    // tmp_dir is dropped here, cleaning up images
    Ok(all_text)
}

/// Split text into chunks for embedding
pub fn chunk_text(text: &str, chunk_size: usize) -> Vec<String> {
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
    // Load the file with extractous
    print!("Loading file...              ");
    io::stdout().flush().ok();
    let start = Instant::now();
    let mut text = load_data(file_path)?;
    println!("{:.2}s ({} chars)", start.elapsed().as_secs_f64(), text.len());

    // For PDFs, also run vision model OCR on each page
    if is_pdf(file_path) {
        match pdf_vision_ocr(file_path).await {
            Ok(vision_text) if !vision_text.is_empty() => {
                println!(
                    "Vision OCR total:            {} chars",
                    vision_text.len()
                );
                text.push_str("\n\n[Vision OCR]\n");
                text.push_str(&vision_text);
            }
            Ok(_) => println!("Vision OCR: no text extracted"),
            Err(e) => println!("Vision OCR failed ({}), continuing with extractous text", e),
        }
    }

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
/// Code files are automatically detected and routed to the code-specific pipeline.
pub async fn load_embed_and_store(file_path: &str, force: bool) -> anyhow::Result<usize> {
    // Route code files to the code-specific pipeline
    if let Some(lang) = crate::parser::detect_language(file_path) {
        return crate::handler::code_loader::load_code_and_store(file_path, lang, force).await;
    }

    // Route image files to the image pipeline
    if crate::handler::image_loader::detect_image(file_path) {
        return crate::handler::image_loader::load_image_and_store(file_path, force).await;
    }

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