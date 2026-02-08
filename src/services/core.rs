use adk_model::ollama::{OllamaModel, OllamaConfig};
use adk_agent::{LlmAgent, LlmAgentBuilder};
use anyhow::Error;
use std::sync::Arc;

const OLLAMA_HOST: &str = "http://0.0.0.0:11434";
const MODEL: &str = "llama3.2";

pub async fn get_agent(
    agent_name: &str,
    system_prompt: &str,
    model_name: Option<&str>
) -> Result<LlmAgent, Error> {
    let config: OllamaConfig = OllamaConfig::with_host(
        OLLAMA_HOST,
        model_name.unwrap_or(MODEL)
    );
    let model = OllamaModel::new(config)?;
    Ok(
        LlmAgentBuilder::new(agent_name)
        .instruction(system_prompt)
        .model(
            Arc::new(model)
        ).build()?
    )
}