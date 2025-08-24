use std::vec::Vec;
use std::collections::HashMap;
use async_trait::async_trait;

use super::tool_structs::{SearchCode, OpensearchKnowledgeBase, WebSearch};


#[async_trait]
pub trait BaseTool {

    // Get tool call definition to be used in api
    fn get_tool_call(&self) -> serde_json::Value;
    async fn run_tool(&self, params: serde_json::Value) -> String;
}


pub struct BaseToolCall<'a> {
    pub available_tools: HashMap<&'a str, Box<dyn BaseTool>>,
}

impl<'a> BaseToolCall<'a> {
    pub fn new() -> Self {
        let mut available_tools: HashMap<&str, Box<dyn BaseTool>> = HashMap::new();
        available_tools.insert("search_code", Box::new(SearchCode {}));
        available_tools.insert("search_knowledge_base", Box::new(OpensearchKnowledgeBase {}));
        available_tools.insert("web_search", Box::new(WebSearch {}));

        Self {
            available_tools,
        }

    }

    pub fn get_available_tool_definitions(&self) -> Vec<serde_json::Value> {
        self.available_tools.values().map(|tool| tool.get_tool_call()).collect()
    }

    pub async fn run_respective_tools(&self, mut params: Vec<serde_json::Value>) -> String {
        let mut results = String::new();
        for item in params.iter_mut() {
            let tool_params = item["function"]["arguments"].take();
            let function_name = item["function"]["name"].as_str().unwrap();
            if let Some(tool) = self.available_tools.get(function_name) {
                results.push_str(&tool.run_tool(tool_params).await);
            }
        }
        results
    }

}

