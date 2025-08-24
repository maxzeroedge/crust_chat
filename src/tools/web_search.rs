use std::error::Error;

use serde_json;
use thirtyfour::prelude::*;

use async_trait::async_trait;
use super::base_tool::BaseTool;
use super::tool_structs::WebSearch;

#[async_trait]

impl BaseTool for WebSearch {
    fn get_tool_call(&self) -> serde_json::Value{
        return serde_json::from_str(r#"{
            "type": "function",
            "function": {
                "name": "web_search",
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

    async fn run_tool(&self, params: serde_json::Value) -> String {
        log::info!("The tool to search the web was called with params {:?}", params);
        let response = WebEngine::search_web(params["query"].to_string()).await;
        return response.unwrap();
    }

}

struct WebEngine {}

impl WebEngine {
    async fn search_web(query: String) -> Result<String, Box<dyn Error + Send + Sync>> {
        let mut caps = DesiredCapabilities::chrome();
        caps.set_binary("/run/current-system/sw/bin/google-chrome-stable")?; 
        let driver = WebDriver::new("http://localhost:44295", caps).await?;

        driver.goto("https://duckduckgo.com/").await?;

        let elem_text = driver.query(
            By::Id("searchbox_input")
        ).first().await?;
        // Type in the search terms.
        elem_text.send_keys(query).await?;
        elem_text.send_keys(Key::Enter).await?;


        // Look for header to implicitly wait for the page to load.
        let elem_result = driver.query(By::Id("react-layout")).first().await?.text().await?;

        // Always explicitly close the browser.
        driver.quit().await?;

        Ok(elem_result)
    }

}
