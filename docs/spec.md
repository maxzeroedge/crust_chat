# RA-CL Specification Document

## 1. System Requirements

### 1.1 Runtime Dependencies

| Dependency | Minimum Version | Purpose |
|------------|-----------------|---------|
| PostgreSQL | 14+ | Relational + vector store host |
| pgvector extension | 0.5+ | Vector operations on PostgreSQL |
| Neo4j | 4.4+ | Graph database for code structure |
| Ollama | Any | Embedding and optional LLM inference |
| LM Studio | Any | Alternative LLM inference (optional) |
| pdftoppm (poppler-utils) | Any | PDF to PNG conversion |
| Tesseract | 4+ | OCR (used via extractous) |
| Leptonica | Any | Image processing (used via extractous) |

### 1.2 Build Dependencies

| Dependency | Version | Notes |
|------------|---------|-------|
| Rust toolchain | stable | Edition 2021 (Cargo.toml declares 2024) |
| C/C++ compiler | Any | Required for tree-sitter native bindings |

### 1.3 Environment Variables

All configuration is provided via a `.env` file in the working directory or existing shell environment.

#### Database

| Variable | Default | Description |
|----------|---------|-------------|
| `PG_HOST` | `localhost` | PostgreSQL host |
| `PG_PORT` | `5432` | PostgreSQL port |
| `PG_DATABASE` | — | PostgreSQL database name |
| `PG_USER` | — | PostgreSQL username |
| `PG_PASS` | — | PostgreSQL password |
| `NEO_4J_HOST` | `localhost` | Neo4j host |
| `NEO_4J_BOLT_PORT` | `7687` | Neo4j Bolt port |
| `NEO_4J_DATABASE` | `neo4j` | Neo4j database name |
| `NEO_4J_USER` | `neo4j` | Neo4j username |
| `NEO_4J_PASS` | — | Neo4j password |

#### Embedding Model

| Variable | Example | Description |
|----------|---------|-------------|
| `EMBEDDING_MODEL_HOST` | `http://localhost` | Ollama host for embeddings |
| `EMBEDDING_MODEL_PORT` | `11434` | Ollama port for embeddings |
| `EMBEDDING_MODEL` | `qwen3-embedding:0.6b` | Ollama embedding model name |

#### Chat Model

| Variable | Values | Description |
|----------|--------|-------------|
| `CHAT_PROVIDER` | `ollama` \| `lmstudio` | Select chat provider |
| `CHAT_MODEL_HOST` | — | Provider host URL |
| `CHAT_MODEL_PORT` | — | Provider port |
| `CHAT_MODEL` | `qwen2.5-coder:7b` | Model name |
| `CHAT_API_KEY` | — | API key (LM Studio only) |

#### Vision Model

| Variable | Values | Description |
|----------|--------|-------------|
| `VISION_PROVIDER` | `ollama` \| `lmstudio` | Select vision provider |
| `VISION_MODEL_HOST` | — | Provider host URL |
| `VISION_MODEL_PORT` | — | Provider port |
| `VISION_MODEL` | `qwen3-vl:8b` | Vision model name |
| `VISION_API_KEY` | — | API key (LM Studio only) |

---

## 2. CLI Specification

### 2.1 Command-Line Arguments

```
ra-cl [OPTIONS]

Options:
  --operation <OP>    Operation to perform [required]
                      Values: simple | chat | loader | search
  --path <PATH>       File or directory path (used with loader)
  --query <QUERY>     Search query string (used with search)
  --force             Force re-index even if file already loaded (used with loader)
  -h, --help          Print help
```

### 2.2 Operation Modes

#### `simple`

Direct LLM chat session. Does not use the knowledge base.

Behavior:
- Opens an interactive input loop
- Reads one line at a time from stdin
- Sends the message to the chat model with a fixed system prompt
- Prints the response
- Exits on `exit`, `quit`, or Ctrl+C (second press)

System prompt: `"You are a helpful assistant."`

#### `chat`

Interactive RAG session backed by the knowledge base.

Behavior:
- Verifies both PostgreSQL and Neo4j connections on startup; exits if either fails
- Opens an interactive input loop
- Maintains a conversation history across turns
- Each user message goes through the full RAG pipeline (see Section 5)
- Supports inline commands (see Section 2.3)
- Exits on `exit`, `quit`, or second Ctrl+C

#### `loader`

Batch-ingest a file or directory into the knowledge base.

Behavior:
- If `--path` is a file: ingest that single file
- If `--path` is a directory: walk the tree recursively and ingest all supported files
- Skips unsupported file extensions silently
- Skips the following directories: `target/`, `node_modules/`, `.git/`, `__pycache__/`, `.venv/`, `dist/`, `build/`
- Without `--force`: skips files already present in the vector store
- With `--force`: deletes existing embeddings for the file, then re-indexes

#### `search`

Raw similarity search without LLM generation.

Behavior:
- Embeds `--query` via the embedding model
- Searches the vector store
- Prints ranked results including source_file, entity_type, entity_name, similarity score, and content
- Does not call the chat model

### 2.3 Chat Commands

These commands are typed as messages during a `chat` session.

| Command | Arguments | Description |
|---------|-----------|-------------|
| `/load <path>` | File or directory path | Ingest path into knowledge base (skip existing) |
| `/reload <path>` | File or directory path | Force re-ingest (delete and re-embed existing) |
| `/save <path>` | Output file path | Run code agent on last response, write code to path |
| `/create [path]` | Optional output directory | Run project agent on last response, build project |
| `exit` | — | Exit the chat session |
| `quit` | — | Exit the chat session |

### 2.4 Interrupt Handling

- **First Ctrl+C**: Cancels the current in-progress async operation (embedding, LLM call, etc.). Returns to the input prompt.
- **Second Ctrl+C**: Exits the process.

A cancellation token is passed into async operations. Operations check the token periodically. If cancelled, the operation returns early and prints a cancellation notice.

---

## 3. File Type Support

### 3.1 Code Files (tree-sitter pipeline)

| Language | Extensions |
|----------|-----------|
| Rust | `.rs` |
| Python | `.py` |
| JavaScript | `.js`, `.mjs`, `.cjs`, `.jsx` |
| TypeScript | `.ts`, `.tsx` |
| Java | `.java` |
| Go | `.go` |
| C | `.c`, `.h` |
| C++ | `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx` |

### 3.2 Document Files (extractous pipeline)

`.pdf`, `.doc`, `.docx`, `.ppt`, `.pptx`, `.xls`, `.xlsx`, `.txt`, `.md`, `.csv`, `.json`, `.xml`, `.html`, `.htm`, `.rtf`, `.odt`

### 3.3 Image Files (OCR + vision pipeline)

`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.tiff`, `.tif`, `.bmp`

---

## 4. Database Schema

### 4.1 PostgreSQL — `embeddings` Table

```sql
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS embeddings (
    id          BIGSERIAL PRIMARY KEY,
    content     TEXT NOT NULL,
    embedding   vector(512),
    source_file TEXT,
    entity_type TEXT,
    entity_name TEXT,
    language    TEXT,
    created_at  TIMESTAMP DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS embeddings_hnsw_idx
    ON embeddings USING hnsw (embedding vector_cosine_ops);

CREATE INDEX IF NOT EXISTS embeddings_entity_type_idx
    ON embeddings (entity_type);
```

**Column semantics:**

| Column | Code entities | Document chunks | Image chunks |
|--------|--------------|-----------------|--------------|
| `content` | Entity source code with imports prepended | Raw text chunk | OCR text + vision description |
| `source_file` | Absolute file path | Absolute file path | Absolute file path |
| `entity_type` | `function`, `struct`, `class`, `method`, `enum`, `trait`, `variable`, `constant`, `module`, `import`, `type_alias`, `interface` | NULL | `image` |
| `entity_name` | Qualified name (`file::parent::name`) | NULL | NULL |
| `language` | `rust`, `python`, `javascript`, `typescript`, `java`, `go`, `c`, `cpp` | NULL | NULL |

### 4.2 Neo4j — Graph Schema

#### Node: `:CodeEntity`

```cypher
CREATE CONSTRAINT code_entity_unique
  IF NOT EXISTS FOR (n:CodeEntity)
  REQUIRE n.qualified_name IS UNIQUE;
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `qualified_name` | String (unique) | Canonical identifier: `file_path::parent::name` |
| `name` | String | Short entity name |
| `entity_type` | String | Same values as PostgreSQL `entity_type` |
| `language` | String | Programming language |
| `source_file` | String | Absolute file path |
| `start_line` | Integer | Line number where entity begins |
| `end_line` | Integer | Line number where entity ends |
| `signature` | String | First line / signature of the entity |

**Additional labels applied per entity type:**

`:File`, `:Module`, `:Class`, `:Struct`, `:Enum`, `:Interface`, `:Trait`, `:Function`, `:Method`, `:Variable`, `:Constant`, `:Import`, `:TypeAlias`

#### Relationships

| Type | Direction | Meaning |
|------|-----------|---------|
| `CONTAINS` | parent → child | Enclosing scope contains entity |
| `CALLS` | caller → callee | Function/method invocation |
| `IMPORTS` | file → import | Import/use declaration |
| `INHERITS` | subtype → supertype | Class/struct inheritance |
| `IMPLEMENTS` | type → trait | Trait/interface implementation |
| `TYPE_REFERENCE` | user → type | Type annotation reference |
| `USES` | entity → entity | Generic usage relationship |

---

## 5. Ingestion Pipeline Specification

### 5.1 Code Ingestion

**Input:** Source file path  
**Output:** Embeddings in PostgreSQL + entity graph in Neo4j

**Step 1 — Parse**

```
parse_code_file(path) → ParseResult { entities, relationships }
```

The tree-sitter engine:
1. Reads file bytes
2. Detects language from extension
3. Parses source into a CST using the language grammar
4. Runs the language-specific tree-sitter query
5. For each query match:
   - Extracts entity fields (name, type, content, line range)
   - Resolves parent entity by walking CST ancestors
   - Builds qualified name: `{source_file}::{parent_name}::{entity_name}`
   - Records relationships from call, import, inheritance captures

**Step 2 — Filter**

Skip entities where:
- `entity_type` is `File` or `Import`
- `content.len() < 50`

**Step 3 — Enrich**

For each entity, collect all `Import` entities from the same file and prepend them to the entity's content:

```
{import_1_content}
{import_2_content}
...
--- {entity_type}: {entity_name} ---
{entity_content}
```

**Step 4 — Embed**

Send entity text to Ollama embedding endpoint in batches of 10:

```
POST http://{EMBEDDING_MODEL_HOST}:{EMBEDDING_MODEL_PORT}/api/embeddings
Content-Type: application/json

{
  "model": "{EMBEDDING_MODEL}",
  "prompt": "{entity_text}"
}
```

Response: `{ "embedding": [f64; 512] }`

**Step 5 — Store (vector)**

For each entity + embedding pair:

```sql
INSERT INTO embeddings (content, embedding, source_file, entity_type, entity_name, language)
VALUES ($1, $2, $3, $4, $5, $6)
```

**Step 6 — Store (graph)**

For each entity: `MERGE (n:CodeEntity {qualified_name: $qn}) SET n += $props SET n:{label}`

For each relationship: `MATCH (a:CodeEntity {qualified_name: $from}) MATCH (b:CodeEntity {qualified_name: $to}) MERGE (a)-[:{type}]->(b)`

### 5.2 Document Ingestion

**Input:** Document file path  
**Output:** Embeddings in PostgreSQL

**Step 1 — Extract text**

Primary path (via extractous):
```
extractous::Extractor::new()
  .extract_file_to_string(path) → (text, metadata)
```

Fallback for PDFs if primary returns empty:
1. Run `pdftoppm -r 200 -png {path} /tmp/page` to produce page PNG files
2. For each page PNG: encode to base64, send to vision model (see Section 6.3)
3. Concatenate all page descriptions

**Step 2 — Chunk**

```
chunk_text(text, max_chunk_size=1000) → Vec<String>
```

Algorithm:
1. Split on `\n\n` (paragraph boundaries)
2. If any paragraph exceeds `max_chunk_size`, split on `\n` (line boundaries)
3. If any line still exceeds `max_chunk_size`, include as a single oversized chunk
4. Empty chunks are discarded

**Step 3 — Embed and Store**

Same as code ingestion steps 4–5, but with `entity_type = NULL`, `entity_name = NULL`, `language = NULL`.

### 5.3 Image Ingestion

**Input:** Image file path  
**Output:** Embeddings in PostgreSQL

**Step 1 — OCR**

```
extractous::Extractor::new()
  .extract_file_to_string(path) → (ocr_text, _)
```

**Step 2 — Vision description**

Read file bytes → base64-encode → send to vision model (see Section 6.3)

**Step 3 — Combine**

```
combined = "{ocr_text}\n\n{vision_description}"
```

**Step 4 — Chunk, Embed, Store**

Same as document ingestion steps 2–3, but with `entity_type = "image"`.

### 5.4 Force Re-index Behavior

When `--force` flag is set or `/reload` command is issued:

```sql
DELETE FROM embeddings WHERE source_file = $1;
```

```cypher
MATCH (n:CodeEntity {source_file: $source_file}) DETACH DELETE n;
```

Then proceed with normal ingestion.

---

## 6. External API Specifications

### 6.1 Ollama Embedding API

**Endpoint:** `POST {EMBEDDING_MODEL_HOST}:{EMBEDDING_MODEL_PORT}/api/embeddings`

**Request:**
```json
{
  "model": "qwen3-embedding:0.6b",
  "prompt": "text to embed"
}
```

**Response:**
```json
{
  "embedding": [0.123, -0.456, ...]
}
```

Embedding dimensionality: **512** (fixed by schema)

### 6.2 Ollama Chat API

**Endpoint:** `POST {CHAT_MODEL_HOST}:{CHAT_MODEL_PORT}/api/chat`

**Request:**
```json
{
  "model": "qwen2.5-coder:7b",
  "messages": [
    { "role": "system", "content": "..." },
    { "role": "user", "content": "..." },
    { "role": "assistant", "content": "..." }
  ],
  "stream": false
}
```

**Response:**
```json
{
  "message": {
    "role": "assistant",
    "content": "..."
  }
}
```

### 6.3 Vision Model API

#### Ollama (vision)

Same as Ollama Chat API, with image content embedded as base64 in the message:

```json
{
  "model": "qwen3-vl:8b",
  "messages": [
    {
      "role": "user",
      "content": "Describe this image in detail...",
      "images": ["<base64_string>"]
    }
  ],
  "stream": false
}
```

**Response:** Same as Ollama Chat API.

#### LM Studio (OpenAI-compatible)

**Endpoint:** `POST {VISION_MODEL_HOST}:{VISION_MODEL_PORT}/v1/chat/completions`

**Headers:** `Authorization: Bearer {VISION_API_KEY}`

**Request:**
```json
{
  "model": "qwen3-vl:8b",
  "messages": [
    {
      "role": "user",
      "content": [
        { "type": "text", "text": "Describe this image..." },
        { "type": "image_url", "image_url": { "url": "data:image/png;base64,..." } }
      ]
    }
  ]
}
```

**Response:**
```json
{
  "choices": [
    { "message": { "role": "assistant", "content": "..." } }
  ]
}
```

### 6.4 LM Studio Chat API (OpenAI-compatible)

**Endpoint:** `POST {CHAT_MODEL_HOST}:{CHAT_MODEL_PORT}/v1/chat/completions`

**Headers:** `Authorization: Bearer {CHAT_API_KEY}`

**Request / Response:** Same format as OpenAI's `/v1/chat/completions`.

---

## 7. RAG Pipeline Specification

### 7.1 Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| `FETCH_K` | 20 | Candidate results fetched from vector store before reranking |
| `TOP_K` | 5 | Final results passed to LLM as context |
| `MIN_SIMILARITY` | 0.5 | Minimum cosine similarity for a result to be included |
| `CODE_BOOST` | 0.15 | Score bonus applied to code entity results during reranking |
| Dedup window | 200 chars | First N characters checked for near-duplicate detection |

### 7.2 Query Embedding

```
embed_query(query_string) → Vec<f64>
```

Calls the Ollama embedding API (Section 6.1) with the raw query string.

### 7.3 Vector Search

```sql
SELECT id, content, source_file, entity_type, entity_name,
       1 - (embedding <=> $1::vector) AS similarity
FROM embeddings
WHERE 1 - (embedding <=> $1::vector) >= 0.5
ORDER BY similarity DESC
LIMIT 20;
```

Returns `SearchResult { id, content, source_file, entity_type?, entity_name?, similarity }`.

### 7.4 Reranking

```
rerank(results: Vec<SearchResult>) → Vec<SearchResult>
```

Algorithm:
1. For each result, compute `adjusted_score = similarity + (CODE_BOOST if entity_type in code_types else 0.0)`
   - `code_types = { "function", "method", "struct", "class", "enum", "trait", "interface", "module", "constant", "variable", "type_alias" }`
2. Sort by `adjusted_score` descending
3. Initialize `selected = []`
4. For each result in sorted order:
   - If `result.content[..200]` does not match any already-selected result's first 200 chars: append to `selected`
   - Stop when `selected.len() == TOP_K`
5. Return `selected`

### 7.5 Context Formatting

```
build_context_string(results: Vec<SearchResult>) → String
```

Format:
```
[1] Source: {source_file}
    Type: {entity_type} — {entity_name}
    Similarity: {similarity:.3}
    {content}

[2] Source: ...
```

When `entity_type` or `entity_name` is absent, those lines are omitted.

### 7.6 LLM Preamble

```
You are a helpful assistant. Answer questions using only the provided context.

Context:
{context_string}

Instructions:
- Use ONLY the APIs, functions, types, and patterns shown in the context above.
- The context is the source of truth; do not invent types or functions not present.
- Cite which context snippets you used: [1], [2], etc.
- When writing code, provide complete, working examples.
- If the context does not contain enough information, clearly state what is missing.
```

### 7.7 Response Object

```rust
struct RagResponse {
    answer: String,
    contexts: Vec<SearchResult>,
}
```

The CLI displays `answer` to the user and shows a summary of which files were used as context.

---

## 8. Code Agent Specification

### 8.1 Purpose

Extracts only valid source code from an LLM response, discarding prose, markdown formatting, and citations.

### 8.2 Preamble

```
You are a code extraction assistant. Your task is to output ONLY valid source code.

Rules:
- Output ONLY valid source code — no explanations, no comments, no markdown
- Include ALL necessary imports and dependencies
- Remove ALL markdown code fences (```), backticks, explanatory text, and citation markers
- Combine multiple code snippets into a single, coherent, compilable file
- Ensure the code is complete and compiles/runs without modification
- Do NOT invent new code beyond what was provided
- Do NOT add extra comments or documentation
```

### 8.3 Post-processing

After the LLM returns output:
1. Strip leading/trailing whitespace
2. Remove markdown code fences:
   - Remove lines matching `` ```{lang} `` or ` ``` `
3. If result is non-empty, write to the target file path
4. Return the cleaned code string

---

## 9. Project Agent Specification

### 9.1 Language Detection

```
detect_language_from_code(code: &str) → ProjectLang
```

Heuristic pattern matching (checked in order):

| Language | Patterns |
|----------|---------|
| Rust | `fn main()`, `use std::`, `let mut `, `impl `, `pub fn ` |
| Python | `def main():`, `import `, `from `, `print(`, `if __name__` |
| JavaScript | `function `, `const `, `require(`, `module.exports`, `console.log` |
| TypeScript | `interface `, `type `, `: string`, `: number`, `as ` |
| Go | `func main()`, `package main`, `import "`, `fmt.` |
| Java | `public class`, `System.out`, `public static void`, `import java.` |

Default fallback: Rust

### 9.2 Project Scaffolding

#### Rust

Directory structure:
```
{project_dir}/
├── Cargo.toml
└── src/
    └── main.rs
```

`Cargo.toml` template:
```toml
[package]
name = "generated_project"
version = "0.1.0"
edition = "2021"

[dependencies]
{auto_detected_dependencies}
```

Dependency extraction: parse `use` statements in the code, extract top-level crate name, filter out `std`, `core`, `alloc`, `self`, `super`, `crate`.

#### Python

```
{project_dir}/
└── main.py
```

#### JavaScript

```
{project_dir}/
├── package.json
└── index.js
```

`package.json`: `{ "name": "generated_project", "version": "1.0.0", "main": "index.js" }`

#### TypeScript

```
{project_dir}/
├── tsconfig.json
└── index.ts
```

`tsconfig.json`: strict mode, `es2020` target, `commonjs` module.

#### Go

```
{project_dir}/
├── go.mod
└── main.go
```

`go.mod`: `module generated_project`, `go 1.21`

#### Java

```
{project_dir}/
└── Main.java
```

### 9.3 Build Commands

| Language | Command |
|----------|---------|
| Rust | `cargo check` |
| Python | `python3 -m py_compile main.py` |
| JavaScript | `node --check index.js` |
| TypeScript | `npx tsc --noEmit` |
| Go | `go build .` |
| Java | `javac Main.java` |

### 9.4 Build-Repair Loop

```
for attempt in 0..3:
    run build_command(project_dir, lang)
    if exit_code == 0:
        print "Build succeeded"
        break
    else:
        error_output = stderr
        repair_prompt = f"""
The following code has build errors. Fix them.

Code:
{code}

Build errors:
{error_output}

Output ONLY the corrected source code.
"""
        code = chat_with_provider(repair_prompt, code_agent_preamble, [])
        write code to project file
```

If all 3 attempts fail, return the project directory with the last attempted code (not an error — the user can inspect and fix manually).

---

## 10. Code Entity Data Model

### 10.1 CodeEntity

```rust
struct CodeEntity {
    qualified_name: String,   // "file_path::parent_name::entity_name"
    name: String,             // Short name only
    entity_type: EntityType,
    language: String,
    source_file: String,      // Absolute path
    content: String,          // Full source text of the entity
    start_line: usize,
    end_line: usize,
    parent: Option<String>,   // Qualified name of parent entity
    signature: Option<String>,// First line / signature
}
```

### 10.2 EntityType Values

`File`, `Module`, `Class`, `Struct`, `Enum`, `Interface`, `Trait`, `Function`, `Method`, `Variable`, `Constant`, `Import`, `TypeAlias`

### 10.3 CodeRelationship

```rust
struct CodeRelationship {
    from_qualified_name: String,
    to_qualified_name: String,
    relationship_type: RelationshipType,
}
```

### 10.4 RelationshipType Values

`Contains`, `Calls`, `Imports`, `Inherits`, `Implements`, `TypeReference`, `Uses`

---

## 11. Message Format

Messages throughout the system use a common format for LLM interactions:

```rust
struct Message {
    role: MessageRole,         // "user" | "assistant" | "tool"
    content: String,
    files: Option<Vec<EncodedFile>>,  // For multimodal messages
}

struct EncodedFile {
    file_name: String,
    data: String,              // Base64-encoded file bytes
}
```

Conversation history is a `Vec<Message>` that grows with each turn. The history is passed with every LLM call to maintain context across turns.

---

## 12. Timing and Logging

The RAG pipeline logs timing for each stage:

| Stage | Log message format |
|-------|--------------------|
| Embedding | `Embedded query in {ms}ms` |
| Vector search | `Searched in {ms}ms` |
| LLM generation | `LLM responded in {ms}ms` |
| Total | `Total RAG pipeline: {ms}ms` |

All timing uses `std::time::Instant`.

The system uses the `log` crate at `info` level for progress, `warn` for non-fatal issues (OCR fallback, missing entities), and `error` for failures.

---

## 13. Error Handling

| Scenario | Behavior |
|----------|----------|
| Database connection failure at startup | Print error, exit process |
| Embedding API unavailable | Return error, surface to CLI |
| Unsupported file extension | Skip silently during directory walk |
| File already in DB (no `--force`) | Skip with info log |
| OCR extraction returns empty | Fall back to vision model |
| Vision model returns empty | Proceed with empty description |
| Build failure after 3 repair attempts | Return project dir with failed code |
| Ctrl+C during operation | Cancel via token, return to prompt |
| Neo4j Community Edition (no multi-DB) | `ensure_database_exists` is a no-op |

---

## 14. Skipped Directories During Directory Walk

When `--path` is a directory, the following paths are excluded from ingestion:

- `target/` (Rust build output)
- `node_modules/`
- `.git/`
- `__pycache__/`
- `.venv/`
- `dist/`
- `build/`

Comparison is done on directory name, not full path, so these are excluded at any nesting depth.
