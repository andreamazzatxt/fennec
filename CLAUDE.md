# Fennec

macOS menu bar app for AI-powered text correction. Runs as a tray icon with global shortcuts.

## Architecture

- **Backend**: Rust (Tauri v2) — `src-tauri/src/`
  - `lib.rs` — app setup, tray, Tauri commands, tap helper install/uninstall
  - `ai.rs` — HTTP client for AI gateway (reqwest), retry logic
  - `clipboard.rs` — copy/paste/select-all via osascript
  - `config.rs` — read/write `~/.fennec.json` (includes TapConfig)
  - `tap_listener.rs` — Unix socket client, receives slap events, triggers correct_all
  - `ax.rs` — macOS Accessibility API for text read/write
- **Frontend**: TypeScript — `src/`
  - `main.ts` — registers global shortcuts, listens for events
  - `settings.ts` — settings UI with auto-save (shortcuts, general) and per-section save (connection, actions)
- **Slap Helper**: Rust + C — `fennec-tap/`
  - `src/main.rs` — daemon entry point, Unix socket server
  - `src/accel_iokit.c` — C shim for IOKit HID accelerometer access (wakes SPU drivers, registers callbacks)
  - `src/accelerometer.rs` — Rust FFI wrapper for the C shim
  - `src/tap_detector.rs` — slap detection algorithm (threshold + cooldown, calibrated from real data)
- **Workspace**: `Cargo.toml` at root — workspace with members `src-tauri` and `fennec-tap`
- **Config**: `~/.fennec.json` — API key, endpoint, model, shortcuts, custom actions, tapToPolish

## Key Commands

```bash
bun install              # install JS deps
bun run tauri dev        # dev mode with hot reload
bun run tauri build      # production build (.dmg)
cargo build -p fennec-tap --release  # build slap helper separately
cargo test -p fennec-tap # run tap detector tests
```

## Build & Release

```bash
# Requires Node >= 20.19 (use nvm use 22)
# Build signed for auto-update
TAURI_SIGNING_PRIVATE_KEY_PATH=~/.tauri/fennec.key TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" bun run tauri build

# Release: builds DMG, creates signed .tar.gz, generates latest.json, uploads to GitHub
# Bump version in src-tauri/tauri.conf.json, src-tauri/Cargo.toml, AND fennec-tap/Cargo.toml
bash scripts/release.sh
```

**Important**: The tar.gz must use `COPYFILE_DISABLE=1` to avoid macOS `._` resource fork files that break the Tauri updater. The release script handles this.

Signing keys stored at `~/.tauri/fennec.key` (private) and `~/.tauri/fennec.key.pub` (public).
Key format is rsign2 (Tauri's format), not standard minisign. Sign with `bun run tauri signer sign`.
Public key is embedded in `src-tauri/tauri.conf.json` under `plugins.updater.pubkey`.

## AI Gateway

Uses Radicalbit AI Gateway (OpenAI-compatible API):
- Endpoint and model route configured per-user in `~/.fennec.json`
- Has guardrails that may block text with typos (returns `guardrail_error`)

## Shortcuts (defaults)

- `Cmd+Shift+.` — correct selected text
- `Cmd+Shift+,` — select all + correct
- `Cmd+Shift+L` — action menu for selection
- `Cmd+Shift+'` — select all + action menu
- `Cmd+Shift+Z` — undo last AI change

## Slap to Polish

Optional feature using the Apple Silicon accelerometer (Bosch BMI286 via IOKit HID).

- **Helper daemon**: `fennec-tap` runs as a launchd daemon at `/usr/local/bin/fennec-tap`
- **Plist**: `/Library/LaunchDaemons/ai.fennec.tap.plist` (requires root, installed via osascript)
- **Communication**: Unix domain socket at `/tmp/fennec-tap.sock`
- **C shim**: Required because IOKit HID from Rust FFI alone doesn't work reliably. The C code:
  1. Wakes SPU drivers (`SensorPropertyReportingState`, `SensorPropertyPowerState`, `ReportInterval`)
  2. Opens all HID devices with usage page 0xFF00
  3. Registers input report callbacks on accelerometer devices (usage 3 and 255)
- **Detection**: Single strong slap (threshold 0.20g for medium). Calibrated from real data — noise floor ~0.014g, light taps ~0.06g, slaps 0.3-0.8g
- **Sensitivity levels**: Low (0.30g), Medium (0.20g), High (0.12g)
- **Apple Silicon only** (M1+), requires admin password on first enable

## Settings UI

- **Shortcuts, General**: auto-save on every change
- **Connection**: per-provider Save button (inside each accordion), disabled until edited
- **Actions**: per-action Save button (inside each card), disabled until edited
- No global footer/Save button

## Notes

- App requires Accessibility permissions (for reading/writing text via AX APIs)
- Clipboard fallback for web apps that don't support AX write
- Select-all uses `Cmd+Up` then `Cmd+Shift+Down` to avoid menu bar flash
- Tray icon animates (ear wiggle) during AI processing — frames at `src-tauri/icons/tray_anim_*.png`
- The `assets/` dir in repo root has the original Electron-era icons; Tauri uses `src-tauri/icons/`
- Dev mode uses red tray icon (`tray_dev.png`) to distinguish from production
