use std::io::BufRead;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};

const SOCKET_PATH: &str = "/tmp/fennec-tap.sock";

pub fn start(app: AppHandle, running: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            match UnixStream::connect(SOCKET_PATH) {
                Ok(stream) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let reader = std::io::BufReader::new(stream);
                    for line in reader.lines() {
                        if !running.load(Ordering::Relaxed) {
                            break;
                        }
                        match line {
                            Ok(msg) if msg.trim() == "TAP" => {
                                let app_clone = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    let state = app_clone.state::<crate::AppState>();
                                    let _ = crate::execute_action_internal(
                                        &app_clone,
                                        &state,
                                        "correct".into(),
                                        true,
                                        None,
                                    )
                                    .await;
                                });
                            }
                            Ok(_) => {}
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                            Err(_) => break,
                        }
                    }
                }
                Err(_) => {}
            }
            // Reconnect backoff
            if running.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(3));
            }
        }
    });
}
