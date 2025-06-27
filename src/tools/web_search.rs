use serde_json;

use super::base_tool::BaseTool;
use super::tool_structs::WebSearch;

impl BaseTool for WebSearch {
    fn get_tool_call(&self) -> serde_json::Value{
        return serde_json::from_str(r#"{
            "type": "function",
            "function": {
                "name": "search_code",
                "description": "Search the internet, using webscraping for being updated with the information",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "the search query"
                        }
                    }
                }
            }
        }"#).unwrap();
                    // "required": ["city", "country_code"]
    }

    fn run_tool(&self, params: serde_json::Value) -> String {
        // TODO: Implement webdriver
        return format!("The tool to search the web was called with params {:?}", params);
    }
}