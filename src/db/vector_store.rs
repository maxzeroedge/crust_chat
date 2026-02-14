use pgvector::Vector;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use std::env;

use crate::handler::data_loader::EmbeddingResult;

/// Create database if it doesn't exist
async fn ensure_database_exists() -> anyhow::Result<()> {
    dotenvy::dotenv_override().ok();

    let host = env::var("PG_HOST")?;
    let port = env::var("PG_PORT")?;
    let database = env::var("PG_DATABASE")?;
    let user = env::var("PG_USER")?;
    let password = env::var("PG_PASS")?;

    // Connect to default 'postgres' database to create our target database
    let admin_url = format!(
        "postgres://{}:{}@{}:{}/postgres",
        user, password, host, port
    );

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await?;

    // Check if database exists
    let exists: bool = sqlx::query(
        "SELECT EXISTS(SELECT 1 FROM pg_database WHERE datname = $1)"
    )
    .bind(&database)
    .fetch_one(&admin_pool)
    .await?
    .get(0);

    if !exists {
        println!("Creating database '{}'...", database);
        // CREATE DATABASE cannot be parameterized, but we control the value from env
        sqlx::query(&format!("CREATE DATABASE \"{}\"", database))
            .execute(&admin_pool)
            .await?;
        println!("Database '{}' created", database);
    }

    admin_pool.close().await;
    Ok(())
}

/// Initialize PostgreSQL connection pool from environment variables
pub async fn init_pool() -> anyhow::Result<PgPool> {
    // Ensure database exists first
    ensure_database_exists().await?;

    dotenvy::dotenv_override().ok();

    let host = env::var("PG_HOST")?;
    let port = env::var("PG_PORT")?;
    let database = env::var("PG_DATABASE")?;
    let user = env::var("PG_USER")?;
    let password = env::var("PG_PASS")?;

    let database_url = format!(
        "postgres://{}:{}@{}:{}/{}",
        user, password, host, port, database
    );

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    Ok(pool)
}

/// Initialize database schema (vector extension and embeddings table)
pub async fn init_schema(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(pool)
        .await?;

    sqlx::query("DROP TABLE IF EXISTS embeddings").execute(pool).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS embeddings (
            id BIGSERIAL PRIMARY KEY,
            content TEXT NOT NULL,
            embedding vector(512),
            source_file TEXT,
            created_at TIMESTAMP DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS embeddings_idx ON embeddings
        USING hnsw (embedding vector_cosine_ops)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Check if a file has already been embedded
pub async fn file_exists(pool: &PgPool, source_file: &str) -> anyhow::Result<bool> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM embeddings WHERE source_file = $1",
    )
    .bind(source_file)
    .fetch_one(pool)
    .await?;

    Ok(count.0 > 0)
}

/// Delete existing embeddings for a file
pub async fn delete_file_embeddings(pool: &PgPool, source_file: &str) -> anyhow::Result<u64> {
    let result = sqlx::query("DELETE FROM embeddings WHERE source_file = $1")
        .bind(source_file)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

/// Store embeddings in PostgreSQL (replaces existing embeddings for the file)
pub async fn store_embeddings(
    pool: &PgPool,
    embeddings: EmbeddingResult,
    source_file: &str,
) -> anyhow::Result<usize> {
    // Delete existing embeddings for this file
    let deleted = delete_file_embeddings(pool, source_file).await?;
    if deleted > 0 {
        println!("Deleted {} existing embeddings for this file", deleted);
    }

    let mut count = 0;

    for (content, embedding_data) in embeddings {
        // Get the first embedding (OneOrMany always has at least one)
        let embedding = embedding_data.first();
        let vec: Vec<f32> = embedding.vec.into_iter().map(|x| x as f32).collect();
        let vector = Vector::from(vec);

        sqlx::query(
            r#"
            INSERT INTO embeddings (content, embedding, source_file)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(&content)
        .bind(&vector)
        .bind(source_file)
        .execute(pool)
        .await?;

        count += 1;
    }

    Ok(count)
}

/// Search result with content, source file, and similarity score
pub struct SearchResult {
    pub id: i64,
    pub content: String,
    pub source_file: String,
    pub similarity: f64,
}

/// Search for similar vectors (for RAG retrieval)
pub async fn search_similar(
    pool: &PgPool,
    query_embedding: Vec<f32>,
    limit: i64,
) -> anyhow::Result<Vec<SearchResult>> {
    let vector = Vector::from(query_embedding);

    let rows: Vec<(i64, String, String, f64)> = sqlx::query_as(
        r#"
        SELECT id, content, COALESCE(source_file, '') as source_file, 1 - (embedding <=> $1) as similarity
        FROM embeddings
        ORDER BY embedding <=> $1
        LIMIT $2
        "#,
    )
    .bind(&vector)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let results = rows
        .into_iter()
        .map(|(id, content, source_file, similarity)| SearchResult {
            id,
            content,
            source_file,
            similarity,
        })
        .collect();

    Ok(results)
}
