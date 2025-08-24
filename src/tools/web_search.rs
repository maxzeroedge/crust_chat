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
        let caps = DesiredCapabilities::chrome();
        let driver = WebDriver::new("https://google.com", caps).await?;

        let elem_text = driver.query(
            By::Css("textarea")
        ).first().await?;
        // Type in the search terms.
        elem_text.send_keys(query).await?;
        elem_text.send_keys(Key::Enter).await?;


        // Look for header to implicitly wait for the page to load.
        let elem_result = driver.query(By::Css("#search > div > div > div:nth-child(2) > div > div")).first().await?.text().await?;

        // Always explicitly close the browser.
        driver.quit().await?;

        Ok(elem_result)
    }

}
