use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub correct: String,
    #[serde(rename = "correctAll")]
    pub correct_all: String,
    pub menu: String,
    #[serde(rename = "menuAll")]
    pub menu_all: String,
    pub undo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAction {
    pub id: String,
    pub label: String,
    pub subtitle: String,
    pub prompt: String,
    pub shortcut: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FennecConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub endpoint: String,
    pub model: String,
    #[serde(rename = "openaiApiKey", default)]
    pub openai_api_key: String,
    #[serde(rename = "openaiModel", default)]
    pub openai_model: String,
    pub shortcuts: ShortcutConfig,
    #[serde(rename = "launchAtLogin", default)]
    pub launch_at_login: bool,
    #[serde(rename = "customActions", default)]
    pub custom_actions: Vec<CustomAction>,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            correct: "CmdOrCtrl+Shift+.".into(),
            correct_all: "CmdOrCtrl+Shift+,".into(),
            menu: "CmdOrCtrl+Shift+L".into(),
            menu_all: "CmdOrCtrl+Shift+'".into(),
            undo: "CmdOrCtrl+Shift+Z".into(),
        }
    }
}

fn default_provider() -> String {
    "radicalbit".into()
}

impl Default for FennecConfig {
    fn default() -> Self {
        Self {
            provider: "radicalbit".into(),
            api_key: String::new(),
            endpoint: "https://ai-gateway.radicalbit.ai/v1/chat/completions".into(),
            model: String::new(),
            openai_api_key: String::new(),
            openai_model: String::new(),
            shortcuts: ShortcutConfig::default(),
            launch_at_login: false,
            custom_actions: vec![],
        }
    }
}

fn config_path() -> PathBuf {
    dirs::home_dir().unwrap().join(".fennec.json")
}

pub fn load_config() -> FennecConfig {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => FennecConfig::default(),
    }
}

pub fn save_config(config: &FennecConfig) -> Result<(), String> {
    let path = config_path();
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}
