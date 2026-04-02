mod ai;
mod ax;
mod clipboard;
mod config;

use config::{FennecConfig, load_config, save_config};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::image::Image;
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
    menu::{MenuBuilder, MenuItemBuilder},
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

struct AppState {
    config: Mutex<FennecConfig>,
    last_original: Mutex<Option<String>>,
    is_loading: AtomicBool,
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
fn check_accessibility() -> bool {
    ax::check_accessibility()
}

async fn execute_action_internal(
    app: &AppHandle,
    state: &AppState,
    action_id: String,
    select_all: bool,
) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();

    if config.api_key.is_empty() {
        return Err("API key not configured".into());
    }

    println!("[fennec] Executing action: {} (select_all: {})", action_id, select_all);

    let read_result = if select_all {
        let text = ax::select_all_text()?;
        ax::ReadResult { text, was_selected: false }
    } else {
        match ax::read_selection_only()? {
            Some(r) => r,
            None => return Err("No text selected".into()),
        }
    };

    if read_result.text.trim().is_empty() {
        return Err("No text selected".into());
    }

    println!("[fennec] Text (selected={}): \"{}...\"", read_result.was_selected, &read_result.text[..read_result.text.len().min(50)]);

    let prompt = build_prompt(&config, &action_id, &read_result.text);

    println!("[fennec] Sending to AI...");

    // Start loading animation
    start_loading(app);

    let result = ai::call_ai_with_retry(&config, &prompt).await;

    // Stop loading animation
    stop_loading(app);

    match result {
        Ok(corrected) => {
            println!("[fennec] Got result: \"{}...\"", &corrected[..corrected.len().min(50)]);
            *state.last_original.lock().unwrap() = Some(read_result.text);
            ax::write_text(&corrected, read_result.was_selected)?;
            clipboard::play_done_sound();
            Ok(())
        }
        Err(e) => {
            eprintln!("[fennec] AI error: {}", e);
            Err(e)
        }
    }
}

#[tauri::command]
async fn execute_action(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    action_id: String,
    select_all: bool,
) -> Result<(), String> {
    execute_action_internal(&app, &state, action_id, select_all).await
}

#[tauri::command]
async fn undo_last(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let original = state.last_original.lock().unwrap().take();
    match original {
        Some(text) => {
            ax::write_text(&text, false)?;
            Ok(())
        }
        None => Err("Nothing to undo".into()),
    }
}

fn start_loading(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.is_loading.store(true, Ordering::SeqCst);

    // Animate tray icon — set_icon must run on the main thread (AppKit requirement)
    let app_clone = app.clone();
    std::thread::spawn(move || {
        let frames: Vec<&str> = vec![
            "icons/tray_anim_0.png",
            "icons/tray_anim_1.png",
            "icons/tray_anim_2.png",
            "icons/tray_anim_1.png",
        ];
        let mut i = 0;
        while app_clone.state::<AppState>().is_loading.load(Ordering::SeqCst) {
            let frame_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(&frames[i % frames.len()]);
            if let Ok(img) = Image::from_path(&frame_path) {
                let app_main = app_clone.clone();
                let _ = app_clone.run_on_main_thread(move || {
                    if let Some(tray) = app_main.tray_by_id("fennec-tray") {
                        let _ = tray.set_icon(Some(img));
                    }
                });
            }
            i += 1;
            std::thread::sleep(std::time::Duration::from_millis(300));
        }
        // Restore original icon
        let icon_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("icons/tray_color.png");
        if let Ok(img) = Image::from_path(&icon_path) {
            let app_main = app_clone.clone();
            let _ = app_clone.run_on_main_thread(move || {
                if let Some(tray) = app_main.tray_by_id("fennec-tray") {
                    let _ = tray.set_icon(Some(img));
                }
            });
        }
    });
}

fn stop_loading(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.is_loading.store(false, Ordering::SeqCst);

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

fn register_shortcuts(app: &AppHandle) {
    let config = app.state::<AppState>().config.lock().unwrap().clone();
    let s = &config.shortcuts;

    // Correct selected text
    let app_handle = app.clone();
    let shortcut = s.correct.clone();
    if let Err(e) = app.global_shortcut().on_shortcut(shortcut.as_str(), move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed { return; }
        let app = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let _ = execute_action_internal(&app, &state, "correct".into(), false).await;
        });
    }) {
        eprintln!("[fennec] Failed to register {}: {}", shortcut, e);
    } else {
        println!("[fennec] Registered: {}", shortcut);
    }

    // Correct all text
    let app_handle = app.clone();
    let shortcut = s.correct_all.clone();
    if let Err(e) = app.global_shortcut().on_shortcut(shortcut.as_str(), move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed { return; }
        let app = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let _ = execute_action_internal(&app, &state, "correct".into(), true).await;
        });
    }) {
        eprintln!("[fennec] Failed to register {}: {}", shortcut, e);
    } else {
        println!("[fennec] Registered: {}", shortcut);
    }

    // Undo
    let app_handle = app.clone();
    let shortcut = s.undo.clone();
    if let Err(e) = app.global_shortcut().on_shortcut(shortcut.as_str(), move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed { return; }
        let app = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let original = state.last_original.lock().unwrap().take();
            if let Some(text) = original {
                let _ = ax::write_text(&text, false);
            }
        });
    }) {
        eprintln!("[fennec] Failed to register {}: {}", shortcut, e);
    } else {
        println!("[fennec] Registered: {}", shortcut);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = load_config();

    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
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
            is_loading: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            update_config,
            check_accessibility,
            execute_action,
            undo_last,
        ])
        .setup(|app| {
            // Check accessibility on startup
            if !ax::check_accessibility() {
                eprintln!("[fennec] Accessibility permission required");
            }

            // Register global shortcuts from Rust
            register_shortcuts(&app.handle().clone());

            // Build tray menu
            let quit = MenuItemBuilder::with_id("quit", "Quit Fennec").build(app)?;
            let correct = MenuItemBuilder::with_id("correct", "Smooth it out").build(app)?;
            let correct_all = MenuItemBuilder::with_id("correct_all", "Smooth all text").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&correct)
                .item(&correct_all)
                .separator()
                .item(&quit)
                .build()?;

            if let Some(tray) = app.tray_by_id("fennec-tray") {
                tray.set_menu(Some(menu))?;
                tray.on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "quit" => app.exit(0),
                        "correct" => {
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let state = app.state::<AppState>();
                                let _ = execute_action_internal(&app, &state, "correct".into(), false).await;
                            });
                        }
                        "correct_all" => {
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let state = app.state::<AppState>();
                                let _ = execute_action_internal(&app, &state, "correct".into(), true).await;
                            });
                        }
                        _ => {}
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
