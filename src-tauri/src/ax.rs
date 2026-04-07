use accessibility_sys::*;
use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use std::ptr;

// GCD FFI for dispatching to the real Cocoa main thread
extern "C" {
    // dispatch_get_main_queue() is a macro that expands to &_dispatch_main_q
    static _dispatch_main_q: std::ffi::c_void;
    fn dispatch_sync_f(
        queue: *const std::ffi::c_void,
        context: *mut std::ffi::c_void,
        work: unsafe extern "C" fn(*mut std::ffi::c_void),
    );
}

/// Run a closure on the real Cocoa main thread via GCD dispatch_sync.
/// Blocks the caller until completion.
fn on_main_thread<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    // Package the closure and result slot into a context
    struct Context<F, R> {
        f: Option<F>,
        result: Option<R>,
    }

    unsafe extern "C" fn trampoline<F, R>(ctx: *mut std::ffi::c_void)
    where
        F: FnOnce() -> R,
    {
        let ctx = &mut *(ctx as *mut Context<F, R>);
        let f = ctx.f.take().unwrap();
        ctx.result = Some(f());
    }

    let mut ctx = Context {
        f: Some(f),
        result: None,
    };

    unsafe {
        dispatch_sync_f(
            &_dispatch_main_q as *const std::ffi::c_void,
            &mut ctx as *mut Context<F, R> as *mut std::ffi::c_void,
            trampoline::<F, R>,
        );
    }

    ctx.result.unwrap()
}

/// Get the PID of the frontmost application via NSWorkspace.
/// This works even when AXFocusedApplication returns kAXErrorNoValue.
fn get_frontmost_pid() -> Option<i32> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    let pid = app.processIdentifier();
    if pid > 0 { Some(pid) } else { None }
}

/// Check if the app has accessibility permissions (silent, no prompt)
pub fn check_accessibility() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted()
}

/// Check with prompt — only call when the user explicitly requests it
pub fn check_accessibility_with_prompt() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted_with_prompt()
}

const AX_RETRY_ATTEMPTS: u32 = 3;
const AX_RETRY_DELAY_MS: u64 = 100;

/// Get the currently focused UI element.
/// Tries AXFocusedApplication first; if that returns kAXErrorNoValue (common on
/// recent macOS with Accessory-policy apps), falls back to NSWorkspace frontmostApplication PID.
/// Retries on kAXErrorCannotComplete (-25212) which is often transient.
unsafe fn get_focused_element() -> Result<AXUIElementRef, String> {
    let mut last_err = String::new();
    for attempt in 0..AX_RETRY_ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(AX_RETRY_DELAY_MS));
            println!("[fennec] AX retry attempt {}/{}", attempt + 1, AX_RETRY_ATTEMPTS);
        }
        match get_focused_element_once() {
            Ok(elem) => return Ok(elem),
            Err(e) => {
                last_err = e;
                // Only retry on kAXErrorCannotComplete
                if !last_err.contains("-25212") {
                    return Err(last_err);
                }
            }
        }
    }
    Err(last_err)
}

unsafe fn get_focused_element_once() -> Result<AXUIElementRef, String> {
    extern "C" { fn pthread_main_np() -> i32; }
    let is_main = pthread_main_np();

    let system = AXUIElementCreateSystemWide();
    let mut focused_app: CFTypeRef = ptr::null_mut();

    let app_attr = CFString::new("AXFocusedApplication");
    let app_err = AXUIElementCopyAttributeValue(
        system,
        app_attr.as_concrete_TypeRef(),
        &mut focused_app,
    );

    let app_source;
    if app_err != kAXErrorSuccess as i32 || focused_app.is_null() {
        // Fallback: get frontmost app PID via NSWorkspace
        if let Some(pid) = get_frontmost_pid() {
            app_source = format!("NSWorkspace PID {}", pid);
            focused_app = AXUIElementCreateApplication(pid) as CFTypeRef;
        } else {
            return Err(format!(
                "No focused application (AXError: {}). Check Accessibility permissions in System Settings > Privacy & Security > Accessibility.",
                app_err
            ));
        }
    } else {
        app_source = "AXFocusedApplication".to_string();
    }

    println!("[fennec] App source: {}, AXFocusedApp err: {}, main_thread: {}", app_source, app_err, is_main);

    // Get the focused UI element from the application
    let elem_attr = CFString::new("AXFocusedUIElement");
    let mut focused: CFTypeRef = ptr::null_mut();
    let err = AXUIElementCopyAttributeValue(
        focused_app as AXUIElementRef,
        elem_attr.as_concrete_TypeRef(),
        &mut focused,
    );

    println!("[fennec] AXFocusedUIElement from app: err={}, null={}", err, focused.is_null());

    if err == kAXErrorSuccess as i32 && !focused.is_null() {
        Ok(focused as AXUIElementRef)
    } else {
        // Also try system-wide as fallback
        let mut focused2: CFTypeRef = ptr::null_mut();
        let err2 = AXUIElementCopyAttributeValue(
            system,
            elem_attr.as_concrete_TypeRef(),
            &mut focused2,
        );
        if err2 == kAXErrorSuccess as i32 && !focused2.is_null() {
            Ok(focused2 as AXUIElementRef)
        } else {
            Err(format!(
                "No focused element (via {}, app AXError: {}, system AXError: {}, main_thread: {})",
                app_source, err, err2, is_main
            ))
        }
    }
}

/// Result of reading text: whether it was a selection or the full value
pub struct ReadResult {
    pub text: String,
    pub was_selected: bool,
}

/// Read only the selected text. Returns None if nothing is selected.
pub fn read_selection_only() -> Result<Option<ReadResult>, String> {
    on_main_thread(read_selection_only_inner)
}

fn read_selection_only_inner() -> Result<Option<ReadResult>, String> {
    unsafe {
        let element = get_focused_element()?;

        let selected_attr = CFString::new("AXSelectedText");
        let mut value: CFTypeRef = ptr::null_mut();

        let err = AXUIElementCopyAttributeValue(
            element,
            selected_attr.as_concrete_TypeRef(),
            &mut value,
        );

        if err == kAXErrorSuccess as i32 && !value.is_null() {
            if let Some(text) = cftype_to_string(value) {
                if !text.is_empty() {
                    println!("[fennec] AX: Read selected text ({} chars)", text.len());
                    return Ok(Some(ReadResult { text, was_selected: true }));
                }
            }
        }

        println!("[fennec] AX: No text selected");
        Ok(None)
    }
}

/// Replace text. Tries AX write first, verifies it worked, falls back to clipboard.
pub fn write_text(new_text: &str, was_selected: bool) -> Result<(), String> {
    let text = new_text.to_string();
    on_main_thread(move || write_text_inner(&text, was_selected))
}

fn write_text_inner(new_text: &str, was_selected: bool) -> Result<(), String> {
    unsafe {
        if let Ok(element) = get_focused_element() {
            let cf_text = CFString::new(new_text);
            let selected_attr = CFString::new("AXSelectedText");

            if was_selected {
                // Try writing via AXSelectedText (replaces selection)
                let err = AXUIElementSetAttributeValue(
                    element,
                    selected_attr.as_concrete_TypeRef(),
                    cf_text.as_CFTypeRef(),
                );

                if err == kAXErrorSuccess as i32 {
                    // Verify by re-reading
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let mut verify: CFTypeRef = ptr::null_mut();
                    let value_attr = CFString::new("AXValue");
                    let _ = AXUIElementCopyAttributeValue(
                        element,
                        value_attr.as_concrete_TypeRef(),
                        &mut verify,
                    );
                    if !verify.is_null() {
                        if let Some(current) = cftype_to_string(verify) {
                            if current.contains(new_text.trim()) {
                                println!("[fennec] Wrote via AX (AXSelectedText)");
                                return Ok(());
                            }
                        }
                    }
                    println!("[fennec] AX write succeeded but text didn't change");
                }
            } else {
                // Try writing via AXValue (replaces all)
                let value_attr = CFString::new("AXValue");
                let err = AXUIElementSetAttributeValue(
                    element,
                    value_attr.as_concrete_TypeRef(),
                    cf_text.as_CFTypeRef(),
                );

                if err == kAXErrorSuccess as i32 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let mut verify: CFTypeRef = ptr::null_mut();
                    let _ = AXUIElementCopyAttributeValue(
                        element,
                        value_attr.as_concrete_TypeRef(),
                        &mut verify,
                    );
                    if !verify.is_null() {
                        if let Some(current) = cftype_to_string(verify) {
                            if current.contains(new_text.trim()) {
                                // Move cursor to end
                                move_cursor_to_end(element, new_text.len());
                                println!("[fennec] Wrote via AX (AXValue)");
                                return Ok(());
                            }
                        }
                    }
                    println!("[fennec] AX write succeeded but text didn't change");
                }
            }
        }
    }

    // Fallback: clipboard + paste
    println!("[fennec] Falling back to clipboard");
    if !was_selected {
        // Use Cmd+A for select all — works better in web apps (WhatsApp, etc.)
        std::process::Command::new("osascript")
            .arg("-e")
            .arg(r#"tell application "System Events" to keystroke "a" using command down"#)
            .output()
            .ok();
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    clipboard_fallback_write(new_text)
}

/// Attempt AX-native write (kept for future use with native-only apps)
#[allow(dead_code)]
fn write_selected_text_ax(new_text: &str) -> Result<(), String> {
    unsafe {
        let element = get_focused_element()?;

        let cf_text = CFString::new(new_text);

        // Try setting AXSelectedText
        let selected_attr = CFString::new("AXSelectedText");
        let err = AXUIElementSetAttributeValue(
            element,
            selected_attr.as_concrete_TypeRef(),
            cf_text.as_CFTypeRef(),
        );

        if err == kAXErrorSuccess as i32 {
            // Verify it actually changed by re-reading
            std::thread::sleep(std::time::Duration::from_millis(100));
            let mut verify: CFTypeRef = ptr::null_mut();
            let value_attr = CFString::new("AXValue");
            let _ = AXUIElementCopyAttributeValue(
                element,
                value_attr.as_concrete_TypeRef(),
                &mut verify,
            );
            if !verify.is_null() {
                if let Some(current) = cftype_to_string(verify) {
                    if current.contains(new_text.trim()) {
                        println!("[fennec] AX: Wrote and verified via AXSelectedText");
                        return Ok(());
                    }
                }
            }
            println!("[fennec] AX: AXSelectedText returned success but text didn't change, falling back");
        } else {
            println!("[fennec] AX: AXSelectedText failed (err: {})", err);
        }

        // Try selecting all text first, then writing via AXSelectedText
        // This keeps cursor at end instead of jumping to start
        let value_attr = CFString::new("AXValue");
        let mut current_value: CFTypeRef = ptr::null_mut();
        let _ = AXUIElementCopyAttributeValue(
            element,
            value_attr.as_concrete_TypeRef(),
            &mut current_value,
        );

        if !current_value.is_null() {
            if let Some(current_text) = cftype_to_string(current_value) {
                // Set selection range to cover all text
                let range_attr = CFString::new("AXSelectedTextRange");
                let len = current_text.len();
                let range = core_foundation::base::CFRange { location: 0, length: len as isize };
                let ax_range = AXValueCreate(
                    kAXValueTypeCFRange,
                    &range as *const _ as *const std::ffi::c_void,
                );
                if !ax_range.is_null() {
                    let _ = AXUIElementSetAttributeValue(
                        element,
                        range_attr.as_concrete_TypeRef(),
                        ax_range as CFTypeRef,
                    );
                    // Now write via AXSelectedText on the full selection
                    let err = AXUIElementSetAttributeValue(
                        element,
                        selected_attr.as_concrete_TypeRef(),
                        cf_text.as_CFTypeRef(),
                    );
                    if err == kAXErrorSuccess as i32 {
                        println!("[fennec] AX: Wrote via select-all + AXSelectedText");
                        return Ok(());
                    }
                }
            }
        }

        // Last resort: set AXValue directly (cursor goes to start)
        let err = AXUIElementSetAttributeValue(
            element,
            value_attr.as_concrete_TypeRef(),
            cf_text.as_CFTypeRef(),
        );

        if err == kAXErrorSuccess as i32 {
            // Move cursor to end
            let range_attr = CFString::new("AXSelectedTextRange");
            let len = new_text.len();
            let range = core_foundation::base::CFRange { location: len as isize, length: 0 };
            let ax_range = AXValueCreate(
                kAXValueTypeCFRange,
                &range as *const _ as *const std::ffi::c_void,
            );
            if !ax_range.is_null() {
                let _ = AXUIElementSetAttributeValue(
                    element,
                    range_attr.as_concrete_TypeRef(),
                    ax_range as CFTypeRef,
                );
            }
            println!("[fennec] AX: Wrote via AXValue + cursor moved to end");
            return Ok(());
        }
        println!("[fennec] AX: All AX methods failed (err: {}), falling back to clipboard", err);
    }

    // Fallback: use clipboard + paste
    clipboard_fallback_write(new_text)
}

/// Write via clipboard + Cmd+V. The user's selection is still active so paste replaces it.
fn clipboard_fallback_write(text: &str) -> Result<(), String> {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    std::process::Command::new("osascript")
        .arg("-e")
        .arg(format!(r#"set the clipboard to "{}""#, escaped))
        .output()
        .map_err(|e| format!("Clipboard write failed: {}", e))?;

    std::process::Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to keystroke "v" using command down"#)
        .output()
        .map_err(|e| format!("Paste failed: {}", e))?;

    println!("[fennec] Wrote via clipboard + paste");
    Ok(())
}

/// Read all text from the focused element (for select-all operations).
/// Falls back to clipboard (Cmd+A, Cmd+C) if AX fails.
pub fn select_all_text() -> Result<String, String> {
    match on_main_thread(select_all_text_inner) {
        Ok(text) => Ok(text),
        Err(e) => {
            println!("[fennec] AX select_all_text failed: {}, trying clipboard fallback", e);
            clipboard_fallback_read()
        }
    }
}

fn select_all_text_inner() -> Result<String, String> {
    unsafe {
        let element = get_focused_element()?;

        // Try AXValue first (works for native text fields)
        let value_attr = CFString::new("AXValue");
        let mut value: CFTypeRef = ptr::null_mut();

        let err = AXUIElementCopyAttributeValue(
            element,
            value_attr.as_concrete_TypeRef(),
            &mut value,
        );

        if err == kAXErrorSuccess as i32 && !value.is_null() {
            if let Some(text) = cftype_to_string(value) {
                if !text.is_empty() {
                    println!("[fennec] AX: Read all text via AXValue ({} chars)", text.len());
                    return Ok(text);
                }
            }
        }

        println!("[fennec] AX: AXValue failed (err={}), trying clipboard fallback", err);
    }

    // Clipboard fallback: Cmd+A, Cmd+C, read pasteboard
    // Works for web apps and elements that don't expose AXValue
    clipboard_fallback_read()
}

/// Read text via clipboard: select all, copy, read pasteboard, then restore.
fn clipboard_fallback_read() -> Result<String, String> {
    // Save current clipboard
    let old_clipboard = std::process::Command::new("pbpaste")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok());

    // Select all (Cmd+A) then copy (Cmd+C)
    std::process::Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events"
    keystroke "a" using command down
    delay 0.1
    keystroke "c" using command down
end tell"#)
        .output()
        .map_err(|e| format!("Select+copy failed: {}", e))?;

    std::thread::sleep(std::time::Duration::from_millis(150));

    // Read clipboard
    let output = std::process::Command::new("pbpaste")
        .output()
        .map_err(|e| format!("pbpaste failed: {}", e))?;
    let text = String::from_utf8_lossy(&output.stdout).to_string();

    // Restore original clipboard
    if let Some(old) = old_clipboard {
        let escaped = old.replace('\\', "\\\\").replace('"', "\\\"");
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(format!(r#"set the clipboard to "{}""#, escaped))
            .output();
    }

    if text.trim().is_empty() {
        return Err("Clipboard fallback: no text read".into());
    }

    println!("[fennec] Read all text via clipboard fallback ({} chars)", text.len());
    Ok(text)
}

/// Move cursor to end of text in element
unsafe fn move_cursor_to_end(element: AXUIElementRef, text_len: usize) {
    let range_attr = CFString::new("AXSelectedTextRange");
    let range = core_foundation::base::CFRange { location: text_len as isize, length: 0 };
    let ax_range = AXValueCreate(
        kAXValueTypeCFRange,
        &range as *const _ as *const std::ffi::c_void,
    );
    if !ax_range.is_null() {
        let _ = AXUIElementSetAttributeValue(
            element,
            range_attr.as_concrete_TypeRef(),
            ax_range as CFTypeRef,
        );
    }
}

/// Convert a CFTypeRef (expected CFString) to a Rust String
unsafe fn cftype_to_string(cf_ref: CFTypeRef) -> Option<String> {
    if cf_ref.is_null() {
        return None;
    }
    let cf_string: CFString = CFString::wrap_under_get_rule(cf_ref as CFStringRef);
    Some(cf_string.to_string())
}
