use neo4rs::*;
use std::env;

use crate::parser::entity::*;

/// Ensure the Neo4j database exists, creating it if necessary.
/// On Community Edition (which only supports a single database), this is a no-op.
async fn ensure_database_exists() -> anyhow::Result<()> {
    dotenvy::dotenv_override().ok();

    let host = env::var("NEO_4J_HOST")?;
    let port = env::var("NEO_4J_BOLT_PORT")?;
    let db = env::var("NEO_4J_DATABASE")?;
    let user = env::var("NEO_4J_USER")?;
    let pass = env::var("NEO_4J_PASS")?;

    let uri = format!("bolt://{}:{}", host, port);

    // Connect to the system database to manage databases
    let config = ConfigBuilder::default()
        .uri(&uri)
        .user(&user)
        .password(&pass)
        .db("system".to_string())
        .build()?;

    let graph = Graph::connect(config).await?;

    // Check if database exists
    let result = graph
        .execute(query("SHOW DATABASES YIELD name WHERE name = $name RETURN name").param("name", db.clone()))
        .await;

    match result {
        Ok(mut rows) => {
            let exists = rows.next().await?.is_some();
            if !exists {
                println!("Creating Neo4j database '{}'...", db);
                graph
                    .run(query(&format!("CREATE DATABASE `{}` IF NOT EXISTS", db)))
                    .await?;
                println!("Neo4j database '{}' created", db);
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
        Err(_) => {
            // Community Edition doesn't support SHOW DATABASES / CREATE DATABASE.
            // The default 'neo4j' database is always available.
        }
    }

    Ok(())
}

pub async fn init_graph() -> anyhow::Result<Graph> {
    dotenvy::dotenv_override().ok();

    let host = env::var("NEO_4J_HOST")?;
    let port = env::var("NEO_4J_BOLT_PORT")?;
    let db = env::var("NEO_4J_DATABASE")?;
    let user = env::var("NEO_4J_USER")?;
    let pass = env::var("NEO_4J_PASS")?;

    let uri = format!("bolt://{}:{}", host, port);

    // Try to ensure database exists (no-op on Community Edition)
    ensure_database_exists().await?;

    let config = ConfigBuilder::default()
        .uri(&uri)
        .user(&user)
        .password(&pass)
        .db(&*db)
        .build()?;

    let graph = Graph::connect(config).await?;
    Ok(graph)
}

pub async fn init_graph_schema(graph: &Graph) -> anyhow::Result<()> {
    let constraints = vec![
        "CREATE CONSTRAINT IF NOT EXISTS FOR (e:CodeEntity) REQUIRE e.qualified_name IS UNIQUE",
        "CREATE INDEX IF NOT EXISTS FOR (e:CodeEntity) ON (e.entity_type)",
        "CREATE INDEX IF NOT EXISTS FOR (e:CodeEntity) ON (e.language)",
        "CREATE INDEX IF NOT EXISTS FOR (e:CodeEntity) ON (e.name)",
    ];
    for q in constraints {
        graph.run(query(q)).await?;
    }
    Ok(())
}

pub async fn delete_file_entities(graph: &Graph, source_file: &str) -> anyhow::Result<()> {
    graph
        .run(
            query("MATCH (e:CodeEntity {source_file: $file}) DETACH DELETE e")
                .param("file", source_file),
        )
        .await?;
    Ok(())
}

pub async fn store_entities(graph: &Graph, entities: &[CodeEntity]) -> anyhow::Result<()> {
    for entity in entities {
        let label = entity.entity_type.neo4j_label();
        let cypher = format!(
            "MERGE (e:CodeEntity:{} {{qualified_name: $qn}})
             SET e.name = $name,
                 e.entity_type = $etype,
                 e.language = $lang,
                 e.source_file = $file,
                 e.start_line = $start,
                 e.end_line = $end_line,
                 e.signature = $sig",
            label
        );
        graph
            .run(
                query(&cypher)
                    .param("qn", entity.qualified_name.clone())
                    .param("name", entity.name.clone())
                    .param("etype", entity.entity_type.to_string())
                    .param("lang", entity.language.to_string())
                    .param("file", entity.source_file.clone())
                    .param("start", entity.start_line as i64)
                    .param("end_line", entity.end_line as i64)
                    .param("sig", entity.signature.clone()),
            )
            .await?;
    }
    Ok(())
}

pub async fn store_relationships(
    graph: &Graph,
    relationships: &[CodeRelationship],
) -> anyhow::Result<()> {
    for rel in relationships {
        let rel_type = rel.relationship_type.neo4j_type();
        let cypher = format!(
            "MATCH (a:CodeEntity {{qualified_name: $from}})
             MATCH (b:CodeEntity {{qualified_name: $to}})
             MERGE (a)-[:{}]->(b)",
            rel_type
        );
        graph
            .run(
                query(&cypher)
                    .param("from", rel.from_qualified_name.clone())
                    .param("to", rel.to_qualified_name.clone()),
            )
            .await?;
    }
    Ok(())
}
