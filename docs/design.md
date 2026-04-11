# RA-CL Design Document

## 1. Overview

RA-CL (Retrieval-Augmented CLI) is a command-line tool that builds a local knowledge base from code and documents, then answers user queries by grounding LLM responses in that knowledge base. It is designed to run entirely on local or self-hosted infrastructure — no cloud APIs are required.

The central design goal is **accurate, cited, context-grounded responses**: rather than relying purely on an LLM's training data, every answer is backed by content the user has explicitly loaded.

---

## 2. Architecture

### 2.1 High-Level Components

```
┌─────────────────────────────────────────────────────────────┐
│                        CLI (main.rs)                        │
│   --operation loader | chat | search | simple               │
└────────┬────────────────────────┬────────────────────────────┘
         │                        │
   ┌─────▼──────┐          ┌──────▼───────┐
   │  Handlers  │          │   Services   │
   │ code_loader│          │    rag.rs    │ ◄── LLM Integration
   │ data_loader│          │  code_agent  │
   │image_loader│          │project_agent │
   └─────┬──────┘          └──────┬───────┘
         │                        │
   ┌─────▼──────┐          ┌──────▼───────┐
   │   Parser   │          │  data_loader │
   │ tree-sitter│          │  embed_query │
   └─────┬──────┘          └──────┬───────┘
         │                        │
         └──────────┬─────────────┘
                    │
          ┌─────────▼──────────┐
          │    Database Layer   │
          │  PostgreSQL+pgvec  │ ◄── Vector Store
          │  Neo4j             │ ◄── Graph Store
          └────────────────────┘
```

### 2.2 External Dependencies

| Dependency | Role |
|------------|------|
| PostgreSQL + pgvector | Vector embedding storage and similarity search |
| Neo4j | Code entity graph (AST nodes + relationships) |
| Ollama | Local embedding model + optional chat/vision model |
| LM Studio | Alternative chat/vision provider (OpenAI-compatible) |
| pdftoppm (poppler) | PDF-to-image conversion for vision OCR |
| extractous | Document text extraction and OCR |

---

## 3. Component Design

### 3.1 CLI Layer (`main.rs`)

The CLI is built with `clap` and supports four operation modes:

- **`simple`**: Direct LLM chat without any knowledge base. Useful for quick questions.
- **`chat`**: Interactive RAG session. Queries are grounded in the knowledge base. Supports inline commands (`/load`, `/save`, `/create`, `/reload`).
- **`loader`**: Batch-loads a file or directory tree into the knowledge base.
- **`search`**: Runs a raw vector similarity search and prints ranked results without calling the LLM.

The chat loop handles `Ctrl+C` gracefully: the first interrupt cancels the current in-progress operation; the second exits the process. A cancellation token (`CancellationToken` from tokio-util) is threaded through async operations.

On startup, the CLI verifies database connectivity for both PostgreSQL and Neo4j before entering any operation.

### 3.2 Handler Layer

Handlers are responsible for ingesting files into the knowledge base. Routing is determined by file extension.

#### 3.2.1 Code Loader (`handler/code_loader.rs`)

Handles source code files supported by tree-sitter. The pipeline:

1. **Parse** — Feed source to the tree-sitter parser to obtain a list of `CodeEntity` objects (functions, structs, classes, methods, etc.) and `CodeRelationship` edges (CALLS, INHERITS, IMPLEMENTS, etc.).
2. **Filter** — Skip trivial entities (`File`, `Import` types, content shorter than 50 chars).
3. **Enrich** — Prepend file-level import statements to each entity's content to preserve dependency context.
4. **Embed** — Batch the enriched text through the Ollama embedding API (batch size 10, 512 dimensions).
5. **Store (vector)** — Write embeddings to PostgreSQL with entity metadata (entity_type, entity_name, language, source_file).
6. **Store (graph)** — Write entity nodes and relationship edges to Neo4j.

#### 3.2.2 Document Loader (`handler/data_loader.rs`)

Handles all non-code, non-image documents (.pdf, .docx, .txt, .md, .json, etc.):

1. **Extract** — Use `extractous` to pull raw text from the document. For PDFs, a fallback path uses `pdftoppm` to render each page as a PNG, then sends it to the vision model for description.
2. **Chunk** — Split the text into segments ≤1000 characters, breaking on paragraph or line boundaries.
3. **Embed** — Batch through Ollama embedding model.
4. **Store** — Write to PostgreSQL with `source_file` metadata.

#### 3.2.3 Image Loader (`handler/image_loader.rs`)

Handles image files (.png, .jpg, .gif, .webp, etc.):

1. **OCR** — Extract any embedded text via `extractous`.
2. **Vision** — Send the raw image bytes (base64-encoded) to the vision model with a description prompt.
3. **Combine** — Concatenate OCR text and vision description.
4. **Chunk, Embed, Store** — Same pipeline as document loader. Entity type is tagged as `"image"`.

### 3.3 Parser Layer (`parser/`)

The parser translates source files into structured entities and relationships using tree-sitter's concrete syntax trees (CST).

#### 3.3.1 Tree-Sitter Engine (`parser/tree_sitter_parser.rs`)

Language is determined from the file extension. The parser:

1. Parses the source using the appropriate tree-sitter grammar.
2. Runs the language-specific S-expression query against the CST.
3. Iterates query matches and classifies captures:
   - `*.name` captures → entity name fields
   - `*.def` captures → full entity definition content
   - `call.name` / `call.method_name` → function/method call sites
   - `import.path` → import/use declarations
   - `inherits.name` / `implements.name` → inheritance edges

4. Resolves parent context by scanning ancestors in the CST (for method-in-class nesting, etc.).
5. Builds qualified names: `source_file::parent_entity::entity_name`.

#### 3.3.2 Language Queries (`parser/queries/`)

Each supported language has its own tree-sitter query file:

| Language | Extensions |
|----------|-----------|
| Rust | `.rs` |
| Python | `.py` |
| JavaScript | `.js`, `.mjs`, `.cjs`, `.jsx` |
| TypeScript | `.ts`, `.tsx` |
| Java | `.java` |
| Go | `.go` |
| C/C++ | `.c`, `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx` |

Each query file exports a single `QUERY` string constant used by the tree-sitter engine.

### 3.4 Database Layer

#### 3.4.1 Vector Store (`db/vector_store.rs`)

PostgreSQL with the `pgvector` extension provides dense vector storage and similarity search.

**Table: `embeddings`**

| Column | Type | Purpose |
|--------|------|---------|
| id | BIGSERIAL | Primary key |
| content | TEXT | Original text content |
| embedding | vector(512) | 512-dimensional dense embedding |
| source_file | TEXT | File path the content came from |
| entity_type | TEXT | `function`, `struct`, `class`, `image`, etc. |
| entity_name | TEXT | Qualified name of the entity |
| language | TEXT | `rust`, `python`, `javascript`, etc. |
| created_at | TIMESTAMP | Insertion time |

**Indices:**
- HNSW index on `embedding` (cosine ops) for approximate nearest-neighbor search
- B-tree index on `entity_type` for filtered queries

Similarity search uses cosine distance, with a minimum threshold of 0.5.

#### 3.4.2 Graph Store (`db/graph_store.rs`)

Neo4j stores the structural relationships extracted from source code.

**Node type: `:CodeEntity`**

Properties: `qualified_name` (unique), `name`, `entity_type`, `language`, `source_file`, `start_line`, `end_line`, `signature`

Additional labels applied per entity type: `:File`, `:Function`, `:Struct`, `:Class`, `:Method`, `:Trait`, `:Enum`, `:Interface`, `:Module`, `:Variable`, `:Constant`, `:Import`, `:TypeAlias`

**Relationship types:** `CONTAINS`, `CALLS`, `IMPORTS`, `INHERITS`, `IMPLEMENTS`, `TYPE_REFERENCE`, `USES`

The graph is currently populated at load time and queried implicitly (graph traversal is not yet wired into the RAG pipeline — the Neo4j layer is infrastructure for future graph-augmented retrieval).

### 3.5 Service Layer

#### 3.5.1 RAG Service (`services/rag.rs`)

The RAG pipeline converts a user query into a grounded LLM response:

```
Query
  │
  ▼ embed_query()
512-dim vector
  │
  ▼ search_similar() → 20 candidates (FETCH_K)
raw results
  │
  ▼ rerank()
  ├─ +0.15 boost for code entities
  ├─ sort descending by adjusted score
  └─ deduplicate (first 200 chars match)
top 5 results (TOP_K)
  │
  ▼ build_context_string()
formatted context with citations [1]...[5]
  │
  ▼ chat_with_provider()
LLM response
  │
  ▼ RagResponse { answer, contexts }
```

The LLM preamble instructs the model to:
- Use only context provided (do not invent)
- Cite which context snippets were used ([1], [2], etc.)
- Provide working code examples if asked
- State what is missing if context is incomplete

Timing is logged for each step: embed, search, LLM, total.

#### 3.5.2 Code Agent (`services/code_agent.rs`)

A second LLM pass that takes raw LLM output and strips everything except code. The agent preamble directs the LLM to:

- Output only valid source code
- Include all imports
- Remove all markdown, explanations, and citations
- Combine multiple snippets into a single coherent file
- Not invent new code beyond what was provided

Output is written to a user-specified file path.

#### 3.5.3 Project Agent (`services/project_agent.rs`)

Orchestrates end-to-end project creation from an LLM response:

1. Extract clean code via code agent
2. Detect programming language (heuristic pattern matching)
3. Create project directory
4. Scaffold language-appropriate project structure (Cargo.toml, package.json, go.mod, etc.)
5. Attempt to build (up to 3 iterations):
   - Run language-specific build/check command
   - On failure: feed code + error output back to LLM for a fix
   - On success: exit loop

Rust scaffolding includes automatic dependency extraction by scanning `use` statements and mapping them to crate names.

### 3.6 Tool Layer (`tools/`)

A framework for LLM-callable tools following the OpenAI function-calling schema. The `BaseTool` trait defines:
- `get_tool_call()` → JSON schema definition for the LLM
- `run_tool(params)` → execute the tool and return a string result

Currently defined (as stubs): `SearchCode`, `WebSearch`, `ParseDocument`, `OpensearchKnowledgeBase`.

---

## 4. Data Flow

### 4.1 Ingestion

```
File or Directory
  │
  ├── Extension check
  │
  ├─ Code file → tree-sitter → entities + relationships
  │                           → PostgreSQL (embeddings)
  │                           → Neo4j (graph)
  │
  ├─ Image file → OCR + vision model → chunks
  │                                  → PostgreSQL (embeddings)
  │
  └─ Document → extractous / vision OCR → chunks
                                        → PostgreSQL (embeddings)
```

### 4.2 Query

```
User query string
  → Ollama embedding API (512-dim)
  → pgvector cosine search (top 20)
  → Reranking (score boost + dedup)
  → Top 5 context snippets
  → LLM (Ollama or LM Studio)
  → Response with citations
```

### 4.3 Code Save

```
LLM response text
  → Code agent (2nd LLM pass)
  → Strip markdown
  → Write to file
```

### 4.4 Project Creation

```
LLM response text
  → Code agent
  → Language detection
  → Project scaffolding
  → Build loop (max 3 tries, LLM-assisted repair)
  → Project directory
```

---

## 5. Multi-Provider Strategy

The system is designed to work with different LLM providers without code changes:

| Layer | Providers |
|-------|-----------|
| Embedding | Ollama only |
| Chat | Ollama, LM Studio |
| Vision | Ollama, LM Studio |

The `CHAT_PROVIDER` / `VISION_PROVIDER` environment variables select the active provider at runtime. LM Studio uses the OpenAI-compatible `/v1/chat/completions` endpoint with Bearer token authentication. Ollama uses its native `/api/chat` endpoint.

The `rig-core` library provides a unified LLM client interface that abstracts provider differences at the chat level.

---

## 6. Design Decisions & Rationale

### 6.1 Why Both Vector and Graph Databases?

Vector search alone finds semantically similar content but cannot traverse structural relationships. The graph store captures "this function calls that function" or "this class inherits from that class." This enables future features like:
- Traversing call graphs to find all callers of a changed function
- Finding all implementations of a trait
- Understanding module dependencies

The graph store is infrastructure investment — it is populated now for future querying capabilities.

### 6.2 Why 512-Dimensional Embeddings?

The chosen model (`qwen3-embedding:0.6b`) uses Matryoshka representation learning, producing embeddings that can be truncated at any power-of-2 dimension while retaining usefulness. 512 dims balances storage cost against retrieval accuracy for a local knowledge base.

### 6.3 Why FETCH_K=20 Then Rerank to TOP_K=5?

Vector similarity alone can return near-duplicate content (e.g., multiple chunks from the same function) or penalize code even when it is more relevant than prose. Fetching 20 candidates and reranking gives room to:
- Boost code entities (+0.15) since they are typically the most actionable context
- Deduplicate overlapping content
- Ensure diversity in the final 5 results

### 6.4 Why Tree-Sitter Over Regex or Simple Splitting?

Tree-sitter provides language-accurate syntax trees. This means entities are extracted at meaningful semantic boundaries (complete function bodies, class definitions) rather than arbitrary character offsets. This produces much better embedding inputs and allows relationship extraction (calls, inheritance).

### 6.5 Why a 3-Attempt Build Loop?

LLMs generating code often produce code that is 90% correct but has minor compilation errors. A single repair pass usually suffices. Three attempts is a pragmatic upper bound that prevents infinite loops while allowing multi-step fixes (e.g., fixing an import error reveals a type error).

### 6.6 Why Local-First?

All models run on local infrastructure (Ollama) or a self-hosted LM Studio instance. The design avoids sending code to cloud APIs — important for proprietary codebases.

---

## 7. Current Limitations

- **Graph queries not yet integrated into RAG**: Neo4j is populated but the RAG pipeline only uses vector search. Graph-augmented retrieval is planned.
- **Tool stubs**: The tool framework exists but individual tools (web search, OpenSearch) are not implemented.
- **No chunking overlap**: Document chunks do not overlap at boundaries, which can split important context across chunks.
- **Single embedding model**: Only Ollama is supported for embeddings; LM Studio is not yet wired in for embeddings.
- **MCP server scaffolded but not implemented**: `server/mod.rs` exists as a placeholder.
- **Rust 2024 edition in Cargo.toml**: May not be supported by all toolchain versions; intended edition may be 2021.
