use std::vec::Vec;
use serde::{Serialize, Deserialize};

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum MessageRole {
    ASSISTANT,
    USER
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedFile {
    pub file_name: String,
    pub data: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip)]
    pub files: Option<Vec<EncodedFile>>
}
