use std::vec::Vec;

use super::tool_structs::{SearchCode, OpensearchKnowledgeBase, WebSearch};

pub trait BaseTool {
    
    // Get tool call definition to be used in api
    fn get_tool_call(&self) -> serde_json::Value;
    fn run_tool(&self, params: serde_json::Value) -> String;
}

pub fn get_available_tool_definitions() -> Vec<serde_json::Value> {
    return Vec::from([
        SearchCode {}.get_tool_call(),
        OpensearchKnowledgeBase {}.get_tool_call(),
        WebSearch {}.get_tool_call(),
    ]);
}

pub fn run_respective_tools(mut params: Vec<serde_json::Value>) -> String {
    //"tool_calls": Array [Object {"function": Object {"arguments": Object {"language": String("json"), "project": String("python")}, "name": String("search_code")}}]}
    for item in params.iter_mut() {
        let params = item["function"]["arguments"].take();
        let function_name = item["function"]["name"].as_str().unwrap();
        return match function_name {
            "search_code" => SearchCode {}.run_tool(params),
            _ => String::new()
        }
    }
    return String::new();
}