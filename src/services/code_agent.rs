use std::io::{self, Write};
use std::time::Instant;

use crate::services::rag::chat_with_provider;

const CODE_AGENT_PREAMBLE: &str = r#"You are a code extraction agent. Your ONLY job is to extract clean, runnable code from the given text.

Rules:
- Output ONLY valid source code, nothing else
- Include all necessary imports/use statements at the top
- Include all necessary type definitions, structs, enums that the code depends on
- Remove markdown formatting (no ``` blocks, no language tags)
- Remove explanatory text, comments about what the code does, and citations like [1] [2]
- Preserve code comments that are part of the actual source code
- If the text contains multiple code snippets, combine them into a single coherent file
- If the text contains no code at all, output exactly: // No code found
- Ensure the code compiles (add any missing closing braces, semicolons, etc.)
- Do not invent new code — only clean and assemble what is present"#;

/// Strip markdown code fences (```lang ... ```) from LLM output
fn strip_markdown_fences(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();

    // Strip leading fence (e.g. ```rust, ```python, ```)
    if let Some(first) = lines.first() {
        if first.trim().starts_with("```") {
            lines.remove(0);
        }
    }

    // Strip trailing fence
    if let Some(last) = lines.last() {
        if last.trim() == "```" {
            lines.pop();
        }
    }

    lines.join("\n")
}

/// Takes an LLM response, sends it through the code cleaning agent,
/// and writes the cleaned code to a file.
pub async fn extract_and_save(llm_response: &str, output_path: &str) -> anyhow::Result<String> {
    print!("Extracting code...           ");
    io::stdout().flush().ok();
    let start = Instant::now();

    let prompt = format!(
        "Extract the clean, runnable code from this text:\n\n{}",
        llm_response
    );

    let cleaned = chat_with_provider(&prompt, CODE_AGENT_PREAMBLE, vec![]).await?;

    println!("{:.2}s", start.elapsed().as_secs_f64());

    // Strip markdown fences if the LLM still included them
    let cleaned = strip_markdown_fences(&cleaned);

    if cleaned.trim() == "// No code found" {
        println!("No code found in the response.");
        return Ok(cleaned);
    }

    std::fs::write(output_path, &cleaned)?;
    println!("Code saved to: {}", output_path);

    Ok(cleaned)
}
