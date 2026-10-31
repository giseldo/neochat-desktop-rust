use crate::mistral::Message;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MODELS: &[&str] = &[
    "mistral-small-latest",
    "mistral-medium-latest",
    "mistral-large-latest",
    "open-mistral-nemo",
    "deepseek-v4-1-flash-1017",
];

pub const APP_DIR: &str = "NeoChat";
pub const CONFIG_FILE: &str = "config.json";
pub const CONVERSATIONS_FILE: &str = "conversations.json";
pub const SECRET_FILE: &str = "secret.dat";

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeChoice {
    #[default]
    Sistema,
    Claro,
    Escuro,
}

impl ThemeChoice {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sistema => "Sistema",
            Self::Claro => "Claro",
            Self::Escuro => "Escuro",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub model: String,
    #[serde(default)]
    pub theme: ThemeChoice,
    #[serde(default)]
    pub window_size: Option<[f32; 2]>,
    #[serde(default = "default_font_scale")]
    pub font_scale: f32,
    #[serde(default)]
    pub cached_models: Vec<String>,
}

fn default_font_scale() -> f32 {
    1.0
}

impl Default for Config {
    fn default() -> Self {
        let model = std::env::var("MISTRAL_MODEL").unwrap_or_else(|_| MODELS[0].to_string());
        Self {
            model,
            theme: ThemeChoice::Sistema,
            window_size: None,
            font_scale: default_font_scale(),
            cached_models: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub interrupted: bool,
}

impl StoredMessage {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.to_string(),
            content: content.into(),
            timestamp: now(),
            interrupted: false,
        }
    }

    pub fn to_api(&self) -> Message {
        Message::new(&self.role, self.content.clone())
    }

    pub fn is_user(&self) -> bool {
        self.role == "user"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub messages: Vec<StoredMessage>,
}

impl Conversation {
    pub fn new(id: impl Into<String>) -> Self {
        let created = now();
        Self {
            id: id.into(),
            title: "Nova conversa".to_string(),
            created_at: created,
            updated_at: created,
            messages: Vec::new(),
        }
    }

    pub fn title_from_messages(&self) -> Option<String> {
        self.messages
            .iter()
            .filter(|m| m.is_user())
            .filter_map(|m| {
                m.content
                    .lines()
                    .map(strip_markdown)
                    .find(|line| !line.is_empty())
            })
            .next()
            .map(|text| truncate_chars(&text, 42))
    }

    pub fn ensure_title(&mut self) {
        if self.title.trim().is_empty() || self.title == "Nova conversa" {
            if let Some(title) = self.title_from_messages() {
                self.title = title;
            }
        }
    }

    pub fn preview(&self) -> String {
        let Some(last) = self.messages.last() else {
            return "Sem mensagens".to_string();
        };
        let cleaned = last
            .content
            .lines()
            .map(strip_markdown)
            .find(|line| !line.is_empty())
            .unwrap_or_default();
        if cleaned.is_empty() {
            "Sem mensagens".to_string()
        } else {
            truncate_chars(&cleaned, 60)
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = now();
    }

    pub fn to_api_messages(&self) -> Vec<Message> {
        self.messages.iter().map(StoredMessage::to_api).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversations {
    pub version: u32,
    pub active: Option<String>,
    pub items: Vec<Conversation>,
}

impl Default for Conversations {
    fn default() -> Self {
        Self {
            version: 1,
            active: None,
            items: Vec::new(),
        }
    }
}

impl Conversations {
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.items.iter().position(|c| c.id == id)
    }

    pub fn active(&self) -> Option<&Conversation> {
        self.active
            .as_deref()
            .and_then(|id| self.index_of(id).map(|i| &self.items[i]))
    }

    pub fn active_mut(&mut self) -> Option<&mut Conversation> {
        let id = self.active.clone()?;
        let index = self.index_of(&id)?;
        Some(&mut self.items[index])
    }

    pub fn new_conversation(&mut self) -> &mut Conversation {
        let mut id = new_id();
        while self.index_of(&id).is_some() {
            id.push('x');
        }
        let conversation = Conversation::new(id);
        self.active = Some(conversation.id.clone());
        self.items.push(conversation);
        self.items.last_mut().expect("acabou de ser inserida")
    }

    pub fn remove(&mut self, id: &str) {
        self.items.retain(|c| c.id != id);
        if self.active.as_deref() == Some(id) {
            self.active = self.items.first().map(|c| c.id.clone());
        }
    }
}

pub struct Store {
    pub config: Config,
    pub conversations: Conversations,
}

impl Default for Store {
    fn default() -> Self {
        let mut conversations = Conversations::default();
        conversations.new_conversation();
        Self {
            config: Config::default(),
            conversations,
        }
    }
}

impl Store {
    pub fn load() -> Self {
        let dir = data_dir();
        let config = read_json::<Config>(&dir.join(CONFIG_FILE)).unwrap_or_default();
        let mut conversations =
            read_json::<Conversations>(&dir.join(CONVERSATIONS_FILE)).unwrap_or_default();
        if conversations.items.is_empty() {
            conversations.new_conversation();
        } else if conversations.active().is_none() {
            conversations.active = conversations.items.first().map(|c| c.id.clone());
        }
        Self {
            config,
            conversations,
        }
    }

    pub fn save(&self) {
        let dir = data_dir();
        if let Err(err) = write_json(&dir.join(CONFIG_FILE), &self.config) {
            eprintln!("falha ao salvar configuracoes: {err:#}");
        }
        if let Err(err) = write_json(&dir.join(CONVERSATIONS_FILE), &self.conversations) {
            eprintln!("falha ao salvar conversas: {err:#}");
        }
    }
}

pub fn data_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join(APP_DIR);
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        if !home.is_empty() {
            return PathBuf::from(home).join(".neochat");
        }
    }
    PathBuf::from(".").join(".neochat")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(value) => Some(value),
        Err(err) => {
            eprintln!("ignorando {} invalido: {err}", path.display());
            None
        }
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn strip_markdown(line: &str) -> String {
    let trimmed = line
        .trim()
        .trim_start_matches(['#', '>', '-', '*', '+'])
        .trim();
    trimmed
        .chars()
        .filter(|&c| !matches!(c, '*' | '_' | '`' | '~'))
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn truncate_chars(text: &str, max: usize) -> String {
    let mut iter = text.chars();
    let truncated: String = iter.by_ref().take(max).collect();
    if iter.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn new_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("c{millis:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip() {
        let config = Config {
            model: "mistral-large-latest".to_string(),
            theme: ThemeChoice::Escuro,
            window_size: Some([900.0, 700.0]),
            font_scale: 1.2,
            cached_models: vec!["codestral-latest".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(back.model, config.model);
        assert_eq!(back.theme, ThemeChoice::Escuro);
        assert_eq!(back.window_size, Some([900.0, 700.0]));
        assert_eq!(back.font_scale, 1.2);
        assert_eq!(back.cached_models, vec!["codestral-latest".to_string()]);
    }

    #[test]
    fn conversations_roundtrip() {
        let mut conversations = Conversations::default();
        let conversation = conversations.new_conversation();
        let id = conversation.id.clone();
        conversation
            .messages
            .push(StoredMessage::new("user", "ola mundo"));
        conversation.ensure_title();

        let json = serde_json::to_string(&conversations).unwrap();
        let back: Conversations = serde_json::from_str(&json).unwrap();
        assert_eq!(back.active.as_deref(), Some(id.as_str()));
        assert_eq!(back.items[0].title, "ola mundo");
        assert_eq!(back.items[0].messages[0].content, "ola mundo");
    }

    #[test]
    fn title_comes_from_first_user_message() {
        let mut conversation = Conversation::new("c1");
        conversation
            .messages
            .push(StoredMessage::new("system", "ignorar"));
        conversation
            .messages
            .push(StoredMessage::new("user", "   "));
        conversation
            .messages
            .push(StoredMessage::new("user", "primeira pergunta"));
        conversation.ensure_title();
        assert_eq!(conversation.title, "primeira pergunta");
    }

    #[test]
    fn preview_strips_markdown_formatting() {
        let mut conversation = Conversation::new("c1");
        conversation.messages.push(StoredMessage::new(
            "assistant",
            "\n# **Capítulo 1: Os Arquitetos das Palavras**\nTexto seguinte",
        ));
        assert_eq!(
            conversation.preview(),
            "Capítulo 1: Os Arquitetos das Palavras"
        );
    }

    #[test]
    fn truncation_is_char_safe() {
        let text = "ação ".repeat(30);
        let truncated = truncate_chars(&text, 10);
        assert!(truncated.ends_with('…'));
        assert_eq!(truncated.chars().count(), 11);
    }

    #[test]
    fn removing_active_selects_another() {
        let mut conversations = Conversations::default();
        let first = conversations.new_conversation().id.clone();
        let second = conversations.new_conversation().id.clone();
        assert_eq!(conversations.active.as_deref(), Some(second.as_str()));
        conversations.remove(&second);
        assert_eq!(conversations.active.as_deref(), Some(first.as_str()));
        assert_eq!(conversations.items.len(), 1);
    }
}
