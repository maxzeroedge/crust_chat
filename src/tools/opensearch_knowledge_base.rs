use async_trait::async_trait;
use serde_json;

use super::base_tool::BaseTool;
use super::tool_structs::OpensearchKnowledgeBase;

#[async_trait]

impl BaseTool for OpensearchKnowledgeBase {
    fn get_tool_call(&self) -> serde_json::Value{
        return serde_json::from_str(r#"{
            "type": "function",
            "function": {
                "name": "search_knowledge_base",
                "description": "Search the knowledge base for a particular query",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "query to use with opensearch vector store"
                        },
                        "index": {
                            "type": "string",
                            "description": "The name of the index which corresponds to the context"
                        }
                    }
                }
            }
        }"#).unwrap();
                    // "required": ["city", "country_code"]
    }

    async fn run_tool(&self, params: serde_json::Value) -> String {
        return format!("The tool to search code was called with params {:?}", params);
    }
}
