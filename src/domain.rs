use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemType {
    Text,
    Url,
    Files,
    Image,
    Custom,
}

impl ItemType {
    pub fn icon(&self) -> &'static str {
        match self {
            ItemType::Text => "📝",
            ItemType::Url => "🔗",
            ItemType::Files => "📁",
            ItemType::Image => "🖼️",
            ItemType::Custom => "📄",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub item_type: ItemType,
    pub text_content: Option<String>,
    pub file_paths: Vec<String>,
    pub image_data: Option<Vec<u8>>,
    pub pinned: bool,
    pub captured_at: DateTime<Local>,
}

impl ClipboardItem {
    pub fn new_text(text: String) -> Self {
        let trimmed = text.trim();
        let is_url = trimmed.starts_with("http://") || trimmed.starts_with("https://");
        let item_type = if is_url { ItemType::Url } else { ItemType::Text };

        let first_line = trimmed.lines().next().unwrap_or("").to_string();
        let title = if first_line.chars().count() > 40 {
            format!("{}...", first_line.chars().take(40).collect::<String>())
        } else if first_line.is_empty() {
            "Empty text".to_string()
        } else {
            first_line
        };

        let preview = if trimmed.chars().count() > 100 {
            format!("{}...", trimmed.chars().take(100).collect::<String>())
        } else {
            trimmed.to_string()
        };

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            preview,
            item_type,
            text_content: Some(text),
            file_paths: Vec::new(),
            image_data: None,
            pinned: false,
            captured_at: Local::now(),
        }
    }

    pub fn new_files(paths: Vec<String>) -> Self {
        let count = paths.len();
        let title = if count == 1 {
            std::path::Path::new(&paths[0])
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| paths[0].clone())
        } else {
            format!("{} files", count)
        };

        let preview = paths.join("\n");

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            preview,
            item_type: ItemType::Files,
            text_content: None,
            file_paths: paths,
            image_data: None,
            pinned: false,
            captured_at: Local::now(),
        }
    }
}
