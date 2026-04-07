use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== Fennec AX Diagnostic ===");
    println!("You have 4 seconds — focus a text field and SELECT some text...\n");

    for i in (1..=4).rev() {
        println!("  {}...", i);
        thread::sleep(Duration::from_secs(1));
    }
    println!();

    // Test 1: Frontmost app via System Events
    println!("--- Test 1: Frontmost Application (System Events) ---");
    let output = run_osascript(r#"
tell application "System Events"
    set frontApp to first application process whose frontmost is true
    return name of frontApp & " (PID: " & unix id of frontApp & ")"
end tell
"#);
    println!("  {}", output);

    // Test 2: Focused UI element via attribute
    println!("\n--- Test 2: Focused UI Element ---");
    let output = run_osascript(r#"
tell application "System Events"
    set frontApp to first application process whose frontmost is true
    try
        set fe to value of attribute "AXFocusedUIElement" of frontApp
        return "OK: " & (role of fe) & " / " & (description of fe)
    on error errMsg
        return "ERROR: " & errMsg
    end try
end tell
"#);
    println!("  {}", output);

    // Test 3: Read value from focused element
    println!("\n--- Test 3: AXValue of focused element ---");
    let output = run_osascript(r#"
tell application "System Events"
    set frontApp to first application process whose frontmost is true
    try
        set fe to value of attribute "AXFocusedUIElement" of frontApp
        set v to value of fe
        return "OK: " & (length of v) & " chars"
    on error errMsg
        return "ERROR: " & errMsg
    end try
end tell
"#);
    println!("  {}", output);

    // Test 4: Read selected text
    println!("\n--- Test 4: AXSelectedText ---");
    let output = run_osascript(r#"
tell application "System Events"
    set frontApp to first application process whose frontmost is true
    try
        set fe to value of attribute "AXFocusedUIElement" of frontApp
        set st to value of attribute "AXSelectedText" of fe
        return "OK: '" & st & "'"
    on error errMsg
        return "ERROR: " & errMsg
    end try
end tell
"#);
    println!("  {}", output);

    // Test 5: System-wide focused element
    println!("\n--- Test 5: System-wide AXFocusedUIElement ---");
    let output = run_osascript(r#"
tell application "System Events"
    try
        set fe to value of attribute "AXFocusedUIElement"
        return "OK: " & (role of fe)
    on error errMsg
        return "ERROR: " & errMsg
    end try
end tell
"#);
    println!("  {}", output);

    // Test 6: Clipboard fallback
    println!("\n--- Test 6: Clipboard fallback (Cmd+C -> pbpaste) ---");
    let _ = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            c.stdin.take().unwrap().write_all(b"")?;
            c.wait()
        });

    let _ = Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to keystroke "c" using command down"#)
        .output();
    thread::sleep(Duration::from_millis(500));

    let output = Command::new("pbpaste").output().expect("pbpaste failed");
    let clip = String::from_utf8_lossy(&output.stdout).to_string();
    if !clip.is_empty() {
        println!("  OK: {} chars — '{}'", clip.len(), &clip[..clip.len().min(100)]);
    } else {
        println!("  EMPTY (no selection or copy failed)");
    }

    println!("\n=== Done ===");
}

fn run_osascript(script: &str) -> String {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .expect("osascript failed");
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stdout.is_empty() { stdout } else if !stderr.is_empty() { format!("STDERR: {}", stderr) } else { "No output".to_string() }
}
