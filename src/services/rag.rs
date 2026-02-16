use rig::client::CompletionClient;
use rig::completion::{Chat, Message};
use sqlx::postgres::PgPool;
use std::env;
use std::time::Instant;

use crate::db::vector_store::{search_similar, SearchResult};
use crate::handler::data_loader::embed_query;

const TOP_K: i64 = 10;
const MIN_SIMILARITY: f64 = 0.5;
const RAG_PREAMBLE: &str = r#"You are a helpful assistant that answers questions based on the provided context.
Use the context to answer the user's question. If the context doesn't contain relevant information, say so.
Always cite which context snippet(s) you used by referencing their numbers [1], [2], etc. You must not invent anything new"#;

/// Chat with the LLM using the configured provider (ollama or lmstudio)
async fn chat_with_provider(
    query: &str,
    preamble: &str,
    chat_history: Vec<Message>,
) -> anyhow::Result<String> {
    dotenvy::dotenv_override().ok();

    let provider = env::var("CHAT_PROVIDER").unwrap_or_else(|_| "lmstudio".to_string());
    let host = env::var("CHAT_MODEL_HOST").unwrap_or_else(|_| "localhost".to_string());
    let model = env::var("CHAT_MODEL").unwrap_or_else(|_| "gemma3:12b-it-q4_K_M".to_string());

    match provider.to_lowercase().as_str() {
        "ollama" => {
            let port = env::var("CHAT_MODEL_PORT").unwrap_or_else(|_| "11434".to_string());
            let base_url = format!("http://{}:{}", host, port);

            let client: rig::providers::ollama::Client = rig::providers::ollama::Client::builder()
                .api_key(rig::client::Nothing)
                .base_url(&base_url)
                .build()
                .expect("Failed to create Ollama chat client");

            let agent = client.agent(&model).preamble(preamble).build();
            Ok(agent.chat(query, chat_history).await?)
        }
        "lmstudio" | _ => {
            let port = env::var("CHAT_MODEL_PORT").unwrap_or_else(|_| "1234".to_string());
            let api_key = env::var("CHAT_API_KEY").unwrap_or_else(|_| "lm-studio".to_string());
            let base_url = format!("http://{}:{}/v1", host, port);

            let client: rig::providers::openai::CompletionsClient = rig::providers::openai::CompletionsClient::builder()
                .api_key(&api_key)
                .base_url(&base_url)
                .build()
                .expect("Failed to create LM Studio chat client");

            let agent = client.agent(&model).preamble(preamble).build();
            Ok(agent.chat(query, chat_history).await?)
        }
    }
}

/// Retrieve relevant context from the vector store
pub async fn retrieve_context(pool: &PgPool, query: &str) -> anyhow::Result<(Vec<SearchResult>, f64, f64)> {
    let t = Instant::now();
    let query_embedding = embed_query(query).await?;
    let embed_time = t.elapsed().as_secs_f64();

    let t = Instant::now();
    let results = search_similar(pool, query_embedding, TOP_K, MIN_SIMILARITY).await?;
    let search_time = t.elapsed().as_secs_f64();

    Ok((results, embed_time, search_time))
}

/// Build context string from search results
fn build_context_string(contexts: &[SearchResult]) -> String {
    let mut context_text = String::new();

    for (i, ctx) in contexts.iter().enumerate() {
        let source = if ctx.source_file.is_empty() {
            "unknown".to_string()
        } else {
            std::path::Path::new(&ctx.source_file)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| ctx.source_file.clone())
        };

        let entity_info = match (&ctx.entity_type, &ctx.entity_name) {
            (Some(etype), Some(ename)) => format!(", type: {}, name: {}", etype, ename),
            (Some(etype), None) => format!(", type: {}", etype),
            _ => String::new(),
        };

        context_text.push_str(&format!(
            "[{}] (source: {}, similarity: {:.2}{})\n{}\n\n",
            i + 1,
            source,
            ctx.similarity,
            entity_info,
            ctx.content
        ));
    }

    context_text
}

/// RAG response paired with the contexts used
pub struct RagResponse {
    pub answer: String,
    pub contexts: Vec<SearchResult>,
}

/// Generate a response using RAG with conversation history
pub async fn generate_rag_response(
    pool: &PgPool,
    query: &str,
    chat_history: &mut Vec<Message>,
) -> anyhow::Result<RagResponse> {
    let total_start = Instant::now();

    // Retrieve relevant context
    let (contexts, embed_time, search_time) = retrieve_context(pool, query).await?;

    if contexts.is_empty() {
        return Ok(RagResponse {
            answer: "No relevant context found in the knowledge base.".to_string(),
            contexts: vec![],
        });
    }

    // Build context string
    let context_str = build_context_string(&contexts);

    // Build full preamble with context
    let preamble = format!(
        "{}\n\nCONTEXT:\n{}",
        RAG_PREAMBLE,
        context_str
    );

    let t = Instant::now();
    let answer = chat_with_provider(query, &preamble, chat_history.clone()).await?;
    let llm_time = t.elapsed().as_secs_f64();

    // Append this turn to history
    chat_history.push(Message::user(query));
    chat_history.push(Message::assistant(&answer));

    let total_time = total_start.elapsed().as_secs_f64();
    println!(
        "\n[Profile] embed: {:.2}s | search: {:.2}s | llm: {:.2}s | total: {:.2}s",
        embed_time, search_time, llm_time, total_time
    );

    Ok(RagResponse { answer, contexts })
}
