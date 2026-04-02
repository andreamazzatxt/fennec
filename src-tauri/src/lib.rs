mod ai;
mod clipboard;
mod config;

use config::{FennecConfig, load_config, save_config};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

struct AppState {
    config: Mutex<FennecConfig>,
    last_original: Mutex<Option<String>>,
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> FennecConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn update_config(state: tauri::State<AppState>, new_config: FennecConfig) -> Result<(), String> {
    save_config(&new_config)?;
    *state.config.lock().unwrap() = new_config;
    Ok(())
}

#[tauri::command]
async fn execute_action(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    action_id: String,
    select_all: bool,
) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();

    if config.api_key.is_empty() {
        return Err("API key not configured".into());
    }

    if select_all {
        clipboard::simulate_select_all()?;
    }

    clipboard::simulate_copy()?;

    let selected_text = get_clipboard_text()?;

    if selected_text.trim().is_empty() {
        return Err("No text selected".into());
    }

    let prompt = build_prompt(&config, &action_id, &selected_text);

    let _ = app.emit("fennec:loading", true);

    let result = ai::call_ai_with_retry(&config, &prompt).await;

    let _ = app.emit("fennec:loading", false);

    match result {
        Ok(corrected) => {
            *app.state::<AppState>().last_original.lock().unwrap() = Some(selected_text);
            set_clipboard_text(&corrected)?;
            clipboard::simulate_paste()?;
            clipboard::play_done_sound();
            Ok(())
        }
        Err(e) => {
            let _ = app.emit("fennec:error", &e);
            Err(e)
        }
    }
}

#[tauri::command]
async fn undo_last(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let original = state.last_original.lock().unwrap().take();
    match original {
        Some(text) => {
            set_clipboard_text(&text)?;
            clipboard::simulate_paste()?;
            Ok(())
        }
        None => Err("Nothing to undo".into()),
    }
}

fn get_clipboard_text() -> Result<String, String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg("the clipboard")
        .output()
        .map_err(|e| format!("Clipboard read failed: {}", e))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn set_clipboard_text(text: &str) -> Result<(), String> {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    std::process::Command::new("osascript")
        .arg("-e")
        .arg(format!(r#"set the clipboard to "{}""#, escaped))
        .output()
        .map_err(|e| format!("Clipboard write failed: {}", e))?;
    Ok(())
}

fn build_prompt(config: &FennecConfig, action_id: &str, text: &str) -> String {
    let instruction = match action_id {
        "correct" => "Fix any grammar, spelling, and punctuation errors in the following text.",
        "formal" => "Rewrite the following text in a formal, professional tone.",
        "informal" => "Rewrite the following text in a casual, friendly tone.",
        "concise" => "Make the following text more concise while keeping its meaning.",
        _ => {
            if let Some(custom) = config
                .custom_actions
                .iter()
                .find(|a| format!("custom_{}", a.id) == action_id)
            {
                return format!(
                    "{}\n\nAuto-detect the language and reply in the same language. Return ONLY the rewritten text, nothing else.\n\n{}",
                    custom.prompt, text
                );
            }
            "Fix any grammar, spelling, and punctuation errors in the following text."
        }
    };

    format!(
        "{} Auto-detect the language and reply in the same language. Return ONLY the corrected text, nothing else.\n\n{}",
        instruction, text
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = load_config();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState {
            config: Mutex::new(config),
            last_original: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            execute_action,
            undo_last,
        ])
        .setup(|_app| {
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
