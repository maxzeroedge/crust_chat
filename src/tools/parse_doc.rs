use adk_rust::tool::FunctionTool;
use async_trait::async_trait;
use serde_json::json;

use super::base_tool::BaseTool;
use super::tool_structs::DocumentParser;

#[async_trait]
impl BaseTool for DocumentParser {

    fn get_tool_call(&self) -> serde_json::Value {
        return serde_json::from_str(r#"{}"#).unwrap();
    }

    async fn run_tool(&self, params: serde_json::Value) -> String {
        return format!("Ran the tool with params {:?}", params);
    }

    fn get_tool(&self) -> Option<FunctionTool> {
        Some(FunctionTool::new(
            "parse_document",
            "Reads the documents and stores it in the knowledge base",
            |_ctx, args| async move {
                println!("{:?}", args);
                Ok(json!({
                    "success": true
                }))
            },
        ))
    }
}