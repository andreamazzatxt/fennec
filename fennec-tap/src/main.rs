mod accelerometer;
mod tap_detector;

use std::io::Write;
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tap_detector::TapDetector;

const SOCKET_PATH: &str = "/tmp/fennec-tap.sock";

fn main() {
    // Record mode: log raw data for 10 seconds then exit
    if std::env::args().any(|a| a == "--record") {
        record_mode();
        return;
    }

    let sensitivity = parse_sensitivity();
    eprintln!("fennec-tap: starting with sensitivity={}", sensitivity);

    // Remove stale socket
    let _ = std::fs::remove_file(SOCKET_PATH);

    // Bind socket
    let listener = UnixListener::bind(SOCKET_PATH).unwrap_or_else(|e| {
        eprintln!("fennec-tap: failed to bind socket: {}", e);
        std::process::exit(1);
    });

    // Make socket world-readable/writable so unprivileged Fennec can connect
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(SOCKET_PATH, std::fs::Permissions::from_mode(0o666));
    }

    // Track connected clients
    let clients: Arc<Mutex<Vec<UnixStream>>> = Arc::new(Mutex::new(Vec::new()));

    // Accept connections in a background thread
    let clients_accept = clients.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    eprintln!("fennec-tap: client connected");
                    clients_accept.lock().unwrap().push(s);
                }
                Err(e) => eprintln!("fennec-tap: accept error: {}", e),
            }
        }
    });

    // Create tap detector
    let detector = Arc::new(Mutex::new(TapDetector::new(&sensitivity)));

    // Notify all connected clients of a double tap
    let notify_clients = {
        let clients = clients.clone();
        move || {
            eprintln!("fennec-tap: SLAP detected!");
            let mut clients = clients.lock().unwrap();
            clients.retain_mut(|stream| match stream.write_all(b"TAP\n") {
                Ok(_) => {
                    let _ = stream.flush();
                    true
                }
                Err(_) => {
                    eprintln!("fennec-tap: client disconnected");
                    let _ = stream.shutdown(Shutdown::Both);
                    false
                }
            });
        }
    };

    // Start accelerometer — this blocks on CFRunLoop
    let notify_for_accel = notify_clients.clone();
    match accelerometer::start(move |x, y, z| {
        let mut det = detector.lock().unwrap();
        if det.feed(x, y, z) {
            notify_for_accel();
        }
    }) {
        Ok(()) => {} // blocks forever (CFRunLoop)
        Err(e) => {
            eprintln!("fennec-tap: accelerometer error: {}", e);
            std::process::exit(1);
        }
    }
}

fn record_mode() {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    eprintln!("=== RECORD MODE ===");
    eprintln!("Recording for 10 seconds. Do several double taps on the chassis.");
    eprintln!("Format: time_ms, deviation (|mag - 1.0|)");
    eprintln!("---");

    let start = Instant::now();
    let count = Arc::new(AtomicU64::new(0));
    let count2 = count.clone();

    match accelerometer::start(move |x, y, z| {
        let elapsed = start.elapsed().as_millis();
        if elapsed > 10000 {
            std::process::exit(0);
        }
        let mag = (x * x + y * y + z * z).sqrt();
        let dev = (mag - 1.0).abs();
        let n = count2.fetch_add(1, Ordering::Relaxed);
        // Print every sample
        println!("{},{:.6}", elapsed, dev);
        // Also print spikes to stderr for visibility
        if dev > 0.02 && n > 10 {
            eprintln!("  {:>5}ms  dev={:.4}  {}", elapsed, dev, if dev > 0.1 { "<<<" } else { "" });
        }
    }) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn parse_sensitivity() -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--sensitivity" {
            if let Some(val) = args.get(i + 1) {
                match val.as_str() {
                    "low" | "medium" | "high" => return val.clone(),
                    _ => {}
                }
            }
        }
    }
    "medium".to_string()
}
