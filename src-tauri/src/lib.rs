mod ai;
mod ax;
mod clipboard;
mod config;

use config::{FennecConfig, load_config, save_config};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::image::Image;
use tauri::{
    AppHandle, Manager, WebviewUrl, WebviewWindowBuilder,
    menu::{MenuBuilder, MenuItemBuilder},
};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

struct AppState {
    config: Mutex<FennecConfig>,
    last_original: Mutex<Option<String>>,
    is_loading: AtomicBool,
    logs: Mutex<Vec<String>>,
}

fn app_log(state: &AppState, msg: &str) {
    let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
    let entry = format!("[{}] {}", timestamp, msg);
    eprintln!("{}", entry);
    let mut logs = state.logs.lock().unwrap();
    logs.push(entry);
    // Keep last 200 lines
    if logs.len() > 200 {
        let excess = logs.len() - 200;
        logs.drain(..excess);
    }
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> FennecConfig {
    state.config.lock().unwrap().clone()
}

#[tauri::command]
fn get_logs(state: tauri::State<AppState>) -> Vec<String> {
    state.logs.lock().unwrap().clone()
}

#[tauri::command]
fn update_config(app: AppHandle, state: tauri::State<AppState>, new_config: FennecConfig) -> Result<(), String> {
    save_config(&new_config)?;
    *state.config.lock().unwrap() = new_config;
    unregister_shortcuts(&app);
    register_shortcuts(&app);
    Ok(())
}

#[tauri::command]
fn pause_shortcuts(app: AppHandle) {
    unregister_shortcuts(&app);
}

#[tauri::command]
fn resume_shortcuts(app: AppHandle) {
    register_shortcuts(&app);
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

    let active_key = match config.provider.as_str() {
        "openai" => &config.openai_api_key,
        _ => &config.api_key,
    };
    if active_key.is_empty() {
        return Err("API key not configured".into());
    }

    app_log(state, &format!("Action: {} (select_all: {})", action_id, select_all));

    let read_result = if select_all {
        let text = ax::select_all_text()?;
        ax::ReadResult { text, was_selected: false }
    } else {
        match ax::read_selection_only()? {
            Some(r) => r,
            None => {
                app_log(state, "No text selected");
                return Err("No text selected".into());
            }
        }
    };

    if read_result.text.trim().is_empty() {
        app_log(state, "No text selected (empty)");
        return Err("No text selected".into());
    }

    app_log(state, &format!("Read {} chars (selected={})", read_result.text.len(), read_result.was_selected));

    let prompt = build_prompt(&config, &action_id, &read_result.text);

    app_log(state, "Sending to AI...");

    // Start loading animation
    start_loading(app);

    let result = ai::call_ai_with_retry(&config, &prompt).await;

    // Stop loading animation
    stop_loading(app);

    match result {
        Ok(corrected) => {
            app_log(state, &format!("AI result: {} chars", corrected.len()));
            *state.last_original.lock().unwrap() = Some(read_result.text);
            ax::write_text(&corrected, read_result.was_selected)?;
            app_log(state, "Text written successfully");
            clipboard::play_done_sound();
            Ok(())
        }
        Err(e) => {
            app_log(state, &format!("AI error: {}", e));
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

/// Called from the action menu popup. Closes the popup, waits for focus to return,
/// then executes the action on the previously focused text field.
#[tauri::command]
async fn execute_action_deferred(
    app: AppHandle,
    action_id: String,
    select_all: bool,
) -> Result<(), String> {
    // Close the popup window
    if let Some(win) = app.get_webview_window("action-menu") {
        let _ = win.close();
    }

    // Wait for focus to return to the original app
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let state = app.state::<AppState>();
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

fn show_action_menu(app: &AppHandle, select_all: bool) {
    let app_clone = app.clone();
    let _ = app.run_on_main_thread(move || {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSMenu, NSMenuItem, NSEvent};
        use objc2_foundation::NSString;

        let mtm = unsafe { MainThreadMarker::new_unchecked() };

        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);

        let titles = ["Smooth it out", "More formal", "More casual", "Make it shorter"];

        for title in &titles {
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    mtm.alloc(),
                    &NSString::from_str(title),
                    None,
                    &NSString::from_str(""),
                )
            };
            item.setEnabled(true);
            menu.addItem(&item);
        }

        // Show menu at mouse location — blocks until user picks or dismisses
        let mouse_loc = NSEvent::mouseLocation();
        let picked = menu.popUpMenuPositioningItem_atLocation_inView(
            None,
            mouse_loc,
            None,
        );

        if picked {
            if let Some(selected) = menu.highlightedItem() {
                let title = selected.title().to_string();
                let action_id = match title.as_str() {
                    "Smooth it out" => "correct",
                    "More formal" => "formal",
                    "More casual" => "informal",
                    "Make it shorter" => "concise",
                    _ => return,
                };

                let app_h = app_clone.clone();
                let action = action_id.to_string();
                tauri::async_runtime::spawn(async move {
                    let state = app_h.state::<AppState>();
                    let _ = execute_action_internal(&app_h, &state, action, select_all).await;
                });
            }
        }
    });
}

fn unregister_shortcuts(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
    let state = app.state::<AppState>();
    app_log(&state, "Unregistered all shortcuts");
}

fn register_shortcuts(app: &AppHandle) {
    let state_ref = app.state::<AppState>();
    let config = state_ref.config.lock().unwrap().clone();
    let s = &config.shortcuts;

    app_log(&state_ref, &format!("Registering shortcuts: correct={}, correct_all={}, undo={}, menu={}, menu_all={}",
        s.correct, s.correct_all, s.undo, s.menu, s.menu_all));

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
        app_log(&state_ref, &format!("FAILED to register {}: {}", shortcut, e));
    } else {
        app_log(&state_ref, &format!("Registered: {}", shortcut));
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
        app_log(&state_ref, &format!("FAILED to register {}: {}", shortcut, e));
    } else {
        app_log(&state_ref, &format!("Registered: {}", shortcut));
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
        app_log(&state_ref, &format!("FAILED to register {}: {}", shortcut, e));
    } else {
        app_log(&state_ref, &format!("Registered: {}", shortcut));
    }

    // Action menu (on selection)
    let app_handle = app.clone();
    let shortcut = s.menu.clone();
    if let Err(e) = app.global_shortcut().on_shortcut(shortcut.as_str(), move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed { return; }
        show_action_menu(&app_handle, false);
    }) {
        app_log(&state_ref, &format!("FAILED to register {}: {}", shortcut, e));
    } else {
        app_log(&state_ref, &format!("Registered: {}", shortcut));
    }

    // Action menu (select all)
    let app_handle = app.clone();
    let shortcut = s.menu_all.clone();
    if let Err(e) = app.global_shortcut().on_shortcut(shortcut.as_str(), move |_app, _shortcut, event| {
        if event.state != ShortcutState::Pressed { return; }
        show_action_menu(&app_handle, true);
    }) {
        app_log(&state_ref, &format!("FAILED to register {}: {}", shortcut, e));
    } else {
        app_log(&state_ref, &format!("Registered: {}", shortcut));
    }

    // Custom action shortcuts
    for action in &config.custom_actions {
        if let Some(ref shortcut_str) = action.shortcut {
            let app_handle = app.clone();
            let shortcut = shortcut_str.clone();
            let action_id = format!("custom_{}", action.id);
            if let Err(e) = app.global_shortcut().on_shortcut(shortcut.as_str(), move |_app, _shortcut, event| {
                if event.state != ShortcutState::Pressed { return; }
                let app = app_handle.clone();
                let id = action_id.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    let _ = execute_action_internal(&app, &state, id, false).await;
                });
            }) {
                app_log(&state_ref, &format!("FAILED to register {}: {}", shortcut, e));
            } else {
                app_log(&state_ref, &format!("Registered custom action: {}", shortcut));
            }
        }
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
            logs: Mutex::new(Vec::new()),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_logs,
            update_config,
            check_accessibility,
            pause_shortcuts,
            resume_shortcuts,
            execute_action,
            execute_action_deferred,
            undo_last,
        ])
        .setup(|app| {
            // Hide from dock — menu bar only
            #[cfg(target_os = "macos")]
            {
                use objc2::MainThreadMarker;
                use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
                let mtm = unsafe { MainThreadMarker::new_unchecked() };
                let ns_app = NSApplication::sharedApplication(mtm);
                ns_app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
            }

            // Check accessibility on startup
            if !ax::check_accessibility() {
                eprintln!("[fennec] Accessibility permission required");
            }

            // Register global shortcuts from Rust
            register_shortcuts(&app.handle().clone());

            // Build tray menu
            let version = app.config().version.clone().unwrap_or_default();
            let version_item = MenuItemBuilder::with_id("version", format!("Fennec v{}", version))
                .enabled(false)
                .build(app)?;
            let settings = MenuItemBuilder::with_id("settings", "Settings...").build(app)?;
            let launch_login = tauri::menu::CheckMenuItemBuilder::with_id("launch_login", "Launch at login")
                .checked(app.state::<AppState>().config.lock().unwrap().launch_at_login)
                .build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit Fennec").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&version_item)
                .separator()
                .item(&settings)
                .item(&launch_login)
                .separator()
                .item(&quit)
                .build()?;

            if let Some(tray) = app.tray_by_id("fennec-tray") {
                tray.set_menu(Some(menu))?;
                tray.on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "quit" => app.exit(0),
                        "launch_login" => {
                            let state = app.state::<AppState>();
                            let mut config = state.config.lock().unwrap();
                            config.launch_at_login = !config.launch_at_login;
                            let enabled = config.launch_at_login;
                            let _ = save_config(&config);
                            drop(config);

                            let autostart = app.autolaunch();
                            if enabled {
                                let _ = autostart.enable();
                            } else {
                                let _ = autostart.disable();
                            }
                        }
                        "settings" => {
                            // Bring app to front (needed for Accessory apps)
                            #[cfg(target_os = "macos")]
                            {
                                use objc2::MainThreadMarker;
                                use objc2_app_kit::NSApplication;
                                let mtm = unsafe { MainThreadMarker::new_unchecked() };
                                let ns_app = NSApplication::sharedApplication(mtm);
                                ns_app.activate();
                            }

                            if let Some(win) = app.get_webview_window("settings") {
                                let _ = win.set_focus();
                            } else {
                                let _ = WebviewWindowBuilder::new(
                                    app,
                                    "settings",
                                    WebviewUrl::App("settings.html".into()),
                                )
                                .title("")
                                .inner_size(480.0, 540.0)
                                .resizable(false)
                                .center()
                                .build();
                            }
                        }
                        _ => {}
                    }
                });
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                // Only prevent exit when it's from closing a window (code is None),
                // not from an explicit app.exit() call (code is Some)
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
