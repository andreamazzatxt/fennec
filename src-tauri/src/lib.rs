mod ai;
mod ax;
mod clipboard;
mod config;
mod tap_listener;

use config::{FennecConfig, load_config, save_config};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
    tap_listener_running: Arc<AtomicBool>,
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
async fn fetch_openai_models(api_key: String) -> Result<Vec<String>, String> {
    #[derive(serde::Deserialize)]
    struct Model {
        id: String,
    }
    #[derive(serde::Deserialize)]
    struct ModelsResponse {
        data: Vec<Model>,
    }

    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.openai.com/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("OpenAI API error: {}", resp.status()));
    }

    let models: ModelsResponse = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
    let mut ids: Vec<String> = models
        .data
        .into_iter()
        .map(|m| m.id)
        .filter(|id| id.starts_with("gpt-") || id.starts_with("o") || id.starts_with("chatgpt-"))
        .collect();
    ids.sort();
    Ok(ids)
}

#[tauri::command]
fn pause_shortcuts(app: AppHandle) {
    unregister_shortcuts(&app);
}

#[tauri::command]
fn resume_shortcuts(app: AppHandle) {
    unregister_shortcuts(&app);
    register_shortcuts(&app);
}

#[tauri::command]
fn check_accessibility() -> bool {
    ax::check_accessibility()
}

#[tauri::command]
fn reset_accessibility() -> Result<String, String> {
    let output = std::process::Command::new("tccutil")
        .args(["reset", "Accessibility", "ai.fennec.app"])
        .output()
        .map_err(|e| format!("Failed to run tccutil: {}", e))?;
    if output.status.success() {
        Ok("Permissions reset. Click Restart to re-grant.".into())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("tccutil failed: {}", stderr))
    }
}

#[tauri::command]
async fn check_for_update(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| format!("{}", e))?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(update.version.clone())),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| format!("{}", e))?;
    match updater.check().await {
        Ok(Some(update)) => {
            let mut bytes = Vec::new();
            update.download_and_install(
                |chunk_len, _content_len| { bytes.extend(std::iter::repeat(0u8).take(chunk_len)); },
                || {},
            ).await.map_err(|e| format!("{}", e))?;
            Ok(())
        }
        Ok(None) => Err("No update available".into()),
        Err(e) => Err(format!("{}", e)),
    }
}

#[tauri::command]
fn restart_app(app: AppHandle) {
    // Use `open` to relaunch from the actual bundle path (works after updates)
    // app.restart() can fail after updater replaces the binary
    let bundle_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("target/release/bundle/macos/Fennec.app");

    // In production, the app lives at /Applications/Fennec.app or similar
    // Get the actual bundle path from the running process
    let app_path = std::env::current_exe()
        .ok()
        .and_then(|p| {
            // Walk up from .app/Contents/MacOS/fennec to .app
            let mut path = p;
            for _ in 0..3 {
                path = path.parent()?.to_path_buf();
            }
            if path.extension().map_or(false, |e| e == "app") {
                Some(path)
            } else {
                None
            }
        });

    if let Some(path) = app_path {
        let _ = std::process::Command::new("open")
            .arg("-n")
            .arg(&path)
            .spawn();
        // Give `open` a moment to launch, then exit
        std::thread::sleep(std::time::Duration::from_millis(500));
        app.exit(0);
    } else {
        // Fallback
        let _ = app.restart();
    }
}


#[tauri::command]
async fn install_tap_helper(app: AppHandle, sensitivity: String) -> Result<(), String> {
    // In dev mode, use the workspace build; in production, use the bundled resource
    let resource_path = if cfg!(debug_assertions) {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("target/release/fennec-tap")
    } else {
        app.path()
            .resource_dir()
            .map_err(|e| format!("Resource dir: {}", e))?
            .join("fennec-tap")
    };

    if !resource_path.exists() {
        return Err(format!(
            "Helper binary not found at {}. Run: cargo build -p fennec-tap --release",
            resource_path.display()
        ));
    }

    // Write plist to a temp file (no privileges needed)
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>ai.fennec.tap</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/fennec-tap</string>
        <string>--sensitivity</string>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>StandardErrorPath</key><string>/tmp/fennec-tap.log</string>
    <key>StandardOutPath</key><string>/tmp/fennec-tap.log</string>
</dict>
</plist>"#,
        sensitivity
    );

    let tmp_plist = std::env::temp_dir().join("ai.fennec.tap.plist");
    std::fs::write(&tmp_plist, &plist).map_err(|e| format!("Write plist: {}", e))?;

    let script = format!(
        "cp '{}' /usr/local/bin/fennec-tap && \
         chmod 755 /usr/local/bin/fennec-tap && \
         codesign --force --sign - /usr/local/bin/fennec-tap && \
         mv '{}' /Library/LaunchDaemons/ai.fennec.tap.plist && \
         chown root:wheel /Library/LaunchDaemons/ai.fennec.tap.plist && \
         chmod 644 /Library/LaunchDaemons/ai.fennec.tap.plist && \
         launchctl unload /Library/LaunchDaemons/ai.fennec.tap.plist 2>/dev/null; \
         launchctl load /Library/LaunchDaemons/ai.fennec.tap.plist",
        resource_path.display(),
        tmp_plist.display()
    );

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "do shell script \"{}\" with administrator privileges",
            script.replace('\\', "\\\\").replace('"', "\\\"")
        ))
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Install failed: {}", stderr));
    }

    // Start the tap listener in the main app
    let state = app.state::<AppState>();
    let running = state.tap_listener_running.clone();
    if !running.load(Ordering::Relaxed) {
        running.store(true, Ordering::Relaxed);
        tap_listener::start(app.clone(), running);
    }

    Ok(())
}

#[tauri::command]
async fn uninstall_tap_helper(app: AppHandle) -> Result<(), String> {
    // Stop the listener first
    let state = app.state::<AppState>();
    state.tap_listener_running.store(false, Ordering::Relaxed);

    let script = "launchctl unload /Library/LaunchDaemons/ai.fennec.tap.plist 2>/dev/null; \
                  rm -f /Library/LaunchDaemons/ai.fennec.tap.plist; \
                  rm -f /usr/local/bin/fennec-tap; \
                  rm -f /tmp/fennec-tap.sock";

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "do shell script \"{}\" with administrator privileges",
            script
        ))
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Uninstall failed: {}", stderr));
    }

    Ok(())
}

#[tauri::command]
fn check_tap_helper_status() -> bool {
    std::os::unix::net::UnixStream::connect("/tmp/fennec-tap.sock").is_ok()
}

#[tauri::command]
async fn test_tap_helper() -> Result<String, String> {
    use std::io::BufRead;

    let stream = std::os::unix::net::UnixStream::connect("/tmp/fennec-tap.sock")
        .map_err(|e| format!("Cannot connect to helper: {}", e))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .map_err(|e| e.to_string())?;

    let reader = std::io::BufReader::new(stream);
    for line in reader.lines() {
        match line {
            Ok(msg) if msg.trim() == "TAP" => return Ok("Slap detected!".into()),
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                return Err("Timeout — no double tap detected in 15s".into());
            }
            Err(e) => return Err(format!("Read error: {}", e)),
        }
    }
    Err("Connection closed without detecting a tap".into())
}

#[tauri::command]
fn check_tap_hardware() -> bool {
    std::process::Command::new("sysctl")
        .args(["-n", "hw.optional.arm64"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "1")
        .unwrap_or(false)
}

/// Read text via AX — call this from the shortcut callback thread (not from tokio).
fn read_text_sync(state: &AppState, select_all: bool) -> Result<ax::ReadResult, String> {
    app_log(state, &format!("AX read (select_all: {}, thread: {:?})", select_all, std::thread::current().id()));

    let read_result = if select_all {
        match ax::select_all_text() {
            Ok(text) => {
                app_log(state, &format!("select_all_text OK: {} chars", text.len()));
                ax::ReadResult { text, was_selected: false }
            }
            Err(e) => {
                app_log(state, &format!("select_all_text FAILED: {}", e));
                return Err(e);
            }
        }
    } else {
        match ax::read_selection_only() {
            Ok(Some(r)) => {
                app_log(state, &format!("read_selection_only OK: {} chars", r.text.len()));
                r
            }
            Ok(None) => {
                app_log(state, "No text selected");
                return Err("No text selected".into());
            }
            Err(e) => {
                app_log(state, &format!("read_selection_only FAILED: {}", e));
                return Err(e);
            }
        }
    };

    if read_result.text.trim().is_empty() {
        app_log(state, "No text selected (empty)");
        return Err("No text selected".into());
    }

    Ok(read_result)
}

async fn execute_action_internal(
    app: &AppHandle,
    state: &AppState,
    action_id: String,
    select_all: bool,
    pre_read: Option<ax::ReadResult>,
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

    // Use pre-read text if available (read from shortcut callback thread),
    // otherwise read here (for Tauri command calls)
    let read_result = match pre_read {
        Some(r) => r,
        None => read_text_sync(state, select_all)?,
    };

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
            match ax::write_text(&corrected, read_result.was_selected) {
                Ok(()) => {
                    app_log(state, "Text written successfully");
                    clipboard::play_done_sound();
                    Ok(())
                }
                Err(e) => {
                    app_log(state, &format!("write_text FAILED: {}", e));
                    Err(e)
                }
            }
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
    execute_action_internal(&app, &state, action_id, select_all, None).await
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
    execute_action_internal(&app, &state, action_id, select_all, None).await
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
        // Restore original icon (dev uses red icon)
        let icon_name = if cfg!(debug_assertions) { "icons/tray_dev.png" } else { "icons/tray_color.png" };
        let icon_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(icon_name);
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
    let config = app.state::<AppState>().config.lock().unwrap().clone();
    let _ = app.run_on_main_thread(move || {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSMenu, NSMenuItem, NSEvent};
        use objc2_foundation::NSString;

        let mtm = unsafe { MainThreadMarker::new_unchecked() };

        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);

        // Built-in actions: (label, action_id)
        let mut actions: Vec<(String, String)> = vec![
            ("\u{2728} Smooth it out".into(), "correct".into()),
            ("\u{1F454} More formal".into(), "formal".into()),
            ("\u{1F60E} More casual".into(), "informal".into()),
            ("\u{2702}\u{FE0F} Make it shorter".into(), "concise".into()),
        ];

        // Add custom actions
        for ca in &config.custom_actions {
            let icon = ca.icon.as_deref().unwrap_or("\u{26A1}");
            actions.push((format!("{} {}", icon, ca.label), format!("custom_{}", ca.id)));
        }

        for (label, _) in &actions {
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    mtm.alloc(),
                    &NSString::from_str(label),
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
                let action_id = actions.iter()
                    .find(|(label, _)| *label == title)
                    .map(|(_, id)| id.clone());

                if let Some(action) = action_id {
                    let app_h = app_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_h.state::<AppState>();
                        let _ = execute_action_internal(&app_h, &state, action, select_all, None).await;
                    });
                }
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
        let state = app.state::<AppState>();
        // Read AX on the shortcut callback thread (before spawning to tokio)
        let pre_read = read_text_sync(&state, false).ok();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let _ = execute_action_internal(&app, &state, "correct".into(), false, pre_read).await;
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
        let state = app.state::<AppState>();
        let pre_read = read_text_sync(&state, true).ok();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let _ = execute_action_internal(&app, &state, "correct".into(), true, pre_read).await;
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
        let state = app.state::<AppState>();
        let original = state.last_original.lock().unwrap().take();
        if let Some(text) = original {
            let _ = ax::write_text(&text, false);
        }
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
                let state = app.state::<AppState>();
                let pre_read = read_text_sync(&state, false).ok();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();
                    let _ = execute_action_internal(&app, &state, id, false, pre_read).await;
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
            tap_listener_running: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            get_logs,
            update_config,
            check_accessibility,
            reset_accessibility,
            restart_app,
            check_for_update,
            install_update,
            pause_shortcuts,
            resume_shortcuts,
            execute_action,
            execute_action_deferred,
            undo_last,
            install_tap_helper,
            uninstall_tap_helper,
            check_tap_helper_status,
            check_tap_hardware,
            test_tap_helper,
            fetch_openai_models,
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

            // In dev mode, use a red tray icon to distinguish from production
            if cfg!(debug_assertions) {
                let dev_icon_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("icons/tray_dev.png");
                if let Ok(img) = Image::from_path(&dev_icon_path) {
                    if let Some(tray) = app.tray_by_id("fennec-tray") {
                        let _ = tray.set_icon(Some(img));
                    }
                }
            }

            // Check accessibility on startup (silent check first)
            if !ax::check_accessibility() {
                eprintln!("[fennec] Accessibility permission not granted, showing prompt");
                ax::check_accessibility_with_prompt();
            }

            // Register global shortcuts from Rust
            register_shortcuts(&app.handle().clone());

            // Start tap listener if enabled
            {
                let state = app.state::<AppState>();
                let config = state.config.lock().unwrap();
                if config.tap_to_polish.as_ref().map_or(false, |t| t.enabled) {
                    let running = state.tap_listener_running.clone();
                    running.store(true, Ordering::Relaxed);
                    tap_listener::start(app.handle().clone(), running);
                }
            }

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
