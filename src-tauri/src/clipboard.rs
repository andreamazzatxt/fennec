use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn simulate_copy() -> Result<(), String> {
    // Wait for modifier keys to release
    thread::sleep(Duration::from_millis(300));

    Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to keystroke "c" using command down"#)
        .output()
        .map_err(|e| format!("Copy failed: {}", e))?;

    thread::sleep(Duration::from_millis(200));
    Ok(())
}

pub fn simulate_paste() -> Result<(), String> {
    Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to keystroke "v" using command down"#)
        .output()
        .map_err(|e| format!("Paste failed: {}", e))?;
    Ok(())
}

pub fn simulate_select_all() -> Result<(), String> {
    thread::sleep(Duration::from_millis(300));

    Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to key code 126 using command down"#)
        .arg("-e")
        .arg("delay 0.05")
        .arg("-e")
        .arg(r#"tell application "System Events" to key code 125 using {command down, shift down}"#)
        .output()
        .map_err(|e| format!("Select all failed: {}", e))?;

    thread::sleep(Duration::from_millis(150));
    Ok(())
}

pub fn play_done_sound() {
    let _ = Command::new("afplay")
        .arg("/System/Library/Sounds/Tink.aiff")
        .spawn();
}
