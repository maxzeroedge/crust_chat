### RA-CL

A command line interface for a hybrid RAG implementation that combines a vector store (PostgreSQL + pgvector) and a graph database (Neo4j). It uses different document parsing utilities in Rust to read files based on type, supports reading code by converting it to an Abstract Syntax Tree (tree-sitter), and stores entities into a vector database with their relationships in a graph database.

The ultimate goal is to be able to use the CLI Chat interface as a simplified reference manual.

## Prerequisites

### System Dependencies

| Dependency | Purpose | Install |
|------------|---------|---------|
| PostgreSQL + pgvector | Vector store for embeddings | Docker or system package |
| Neo4j | Graph database for code entity relationships | Docker or system package |
| poppler-utils | PDF page-to-image conversion (`pdftoppm`) for vision OCR | `sudo pacman -S poppler` (Arch) / `sudo apt install poppler-utils` (Debian) |
| libtesseract-dev + libleptonica-dev | OCR for extractous | `sudo pacman -S tesseract` / `sudo apt install libtesseract-dev libleptonica-dev` |

### LLM Providers

RA-CL supports two LLM providers for chat and vision:

| Provider | API Format | Default Port | Notes |
|----------|------------|--------------|-------|
| **LM Studio** | OpenAI-compatible (`/v1/chat/completions`) | 1234 | Requires API key (default: `lm-studio`) |
| **Ollama** | Ollama native (`/api/chat`) | 11434 | No API key needed |

An embedding model is also required (runs on Ollama):
- **qwen3-embedding:0.6b** (512-dim Matryoshka embeddings)

## Configuration

Create a `.env` file in the project root:

```env
# ── Databases ──────────────────────────────────────────
PG_HOST=localhost
PG_PORT=5432
PG_DATABASE=racl_vector
PG_USER=postgresadmin
PG_PASS=postgresadmin

NEO_4J_HOST=localhost
NEO_4J_BOLT_PORT=7687
NEO_4J_DATABASE=neo4j
NEO_4J_USER=neo4j
NEO_4J_PASS=your_password

# ── Embedding Model (Ollama) ──────────────────────────
EMBEDDING_MODEL_HOST=localhost
EMBEDDING_MODEL_PORT=11434
EMBEDDING_MODEL=qwen3-embedding:0.6b

# ── Chat Model ────────────────────────────────────────
CHAT_PROVIDER=lmstudio          # "lmstudio" or "ollama"
CHAT_MODEL_HOST=localhost
CHAT_MODEL_PORT=1234            # 1234 for lmstudio, 11434 for ollama
CHAT_MODEL=gemma3:12b-it-q4_K_M
CHAT_API_KEY=lm-studio          # only used by lmstudio provider

# ── Vision Model ──────────────────────────────────────
VISION_PROVIDER=lmstudio        # "lmstudio" or "ollama"
VISION_MODEL_HOST=localhost
VISION_MODEL_PORT=1234          # 1234 for lmstudio, 11434 for ollama
VISION_MODEL=gemma3:12b-it-q4_K_M
VISION_API_KEY=lm-studio        # only used by lmstudio provider
```

### Quick Provider Switch

**LM Studio:**
```env
CHAT_PROVIDER=lmstudio
CHAT_MODEL_PORT=1234
CHAT_API_KEY=lm-studio
```

**Ollama:**
```env
CHAT_PROVIDER=ollama
CHAT_MODEL_PORT=11434
# CHAT_API_KEY not needed
```

Chat and vision providers can be configured independently.

## Usage

### Operations

```bash
# Start RAG chat (interactive, with conversation history)
cargo run -- --operation chat

# Load a single file into the knowledge base
cargo run -- --operation loader --path "/path/to/file.pdf"

# Load an entire directory (recursive)
cargo run -- --operation loader --path "/path/to/project/"

# Force reload (re-process already indexed files)
cargo run -- --operation loader --path "/path/to/file.pdf" --force

# Search the knowledge base (raw results, no LLM)
cargo run -- --operation search --query "how does authentication work"

# Simple chat (no RAG, direct LLM conversation)
cargo run -- --operation simple
```

### CLI Arguments

| Argument | Short | Description |
|----------|-------|-------------|
| `--operation` | `-o` | Operation to run: `chat`, `loader`, `search`, `simple` |
| `--path` | `-p` | File or directory path (required for `loader`) |
| `--force` | `-f` | Force reload of already processed files |
| `--query` | `-q` | Search query (required for `search`) |

## Supported File Types

### Code Files (tree-sitter AST parsing + graph storage)

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

Code files are parsed into entities (functions, classes, structs, enums, traits, methods, etc.) and relationships (contains, calls, imports, inherits, implements). Entities are embedded and stored in PostgreSQL; relationships are stored in Neo4j.

### Document Files (extractous text extraction)

`pdf`, `doc`, `docx`, `ppt`, `pptx`, `xls`, `xlsx`, `txt`, `md`, `csv`, `json`, `xml`, `html`, `htm`, `rtf`, `odt`

PDF files get additional vision model OCR: each page is converted to an image via `pdftoppm` and sent to the vision model for text extraction, which is combined with the extractous output.

### Image Files (OCR + vision model)

`png`, `jpg`, `jpeg`, `gif`, `webp`, `tiff`, `tif`, `bmp`

Images are processed with both extractous OCR and the vision model to extract text and generate descriptions.

## Architecture

```
load_embed_and_store(file_path)
  ├── Code file     → tree-sitter AST → entities + relationships
  │                   → embed entities → PostgreSQL (pgvector)
  │                   → store graph    → Neo4j
  ├── Image file    → extractous OCR + vision model description
  │                   → combine → chunk → embed → PostgreSQL
  └── Document file → extractous text extraction (with OCR fallback)
                      → vision OCR per page (PDF only, via pdftoppm)
                      → combine → chunk → embed → PostgreSQL
```

```
RAG Chat
  ├── User query → embed → vector similarity search (PostgreSQL)
  ├── Top-K results (min similarity 0.5) → build context
  ├── Context + conversation history → LLM (lmstudio/ollama)
  └── Response with numbered citations [1], [2], etc.
      + printed references with source, type, name, similarity
```
