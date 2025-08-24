use async_trait::async_trait;
use serde_json;

use super::base_tool::BaseTool;
use super::tool_structs::SearchCode;

#[async_trait]

impl BaseTool for SearchCode {
    fn get_tool_call(&self) -> serde_json::Value{
        return serde_json::from_str(r#"{
            "type": "function",
            "function": {
                "name": "search_code",
                "description": "Get snippets from personal codebase for a particular programming language or project",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "language": {
                            "type": "string",
                            "description": "The programming language used in the project"
                        },
                        "project": {
                            "type": "string",
                            "description": "The name of the project"
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
