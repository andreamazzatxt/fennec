# Fennec

macOS menu bar app for AI-powered text correction. Runs as a tray icon with global shortcuts.

## Architecture

- **Backend**: Rust (Tauri v2) — `src-tauri/src/`
  - `lib.rs` — app setup, tray, Tauri commands
  - `ai.rs` — HTTP client for AI gateway (reqwest), retry logic
  - `clipboard.rs` — copy/paste/select-all via osascript
  - `config.rs` — read/write `~/.fennec.json`
- **Frontend**: TypeScript — `src/`
  - `main.ts` — registers global shortcuts, listens for events
- **Config**: `~/.fennec.json` — API key, endpoint, model, shortcuts, custom actions

## Key Commands

```bash
bun install              # install JS deps
bun run tauri dev        # dev mode with hot reload
bun run tauri build      # production build (.dmg)
```

## Build & Release

```bash
# Build signed for auto-update
TAURI_SIGNING_PRIVATE_KEY_PATH=~/.tauri/fennec.key TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" bun run tauri build

# Create GitHub release
gh release create vX.Y.Z src-tauri/target/release/bundle/dmg/*.dmg --title "Fennec vX.Y.Z"
```

Signing keys stored at `~/.tauri/fennec.key` (private) and `~/.tauri/fennec.key.pub` (public).
Public key is embedded in `src-tauri/tauri.conf.json` under `plugins.updater.pubkey`.

## AI Gateway

Uses Radicalbit AI Gateway (OpenAI-compatible API):
- Endpoint and model route configured per-user in `~/.fennec.json`
- Has guardrails that may block text with typos (returns `guardrail_error`)

## Shortcuts (defaults)

- `Cmd+Shift+.` — correct selected text
- `Cmd+Shift+;` — select all + correct
- `Cmd+Shift+L` — action menu for selection
- `Cmd+Shift+'` — select all + action menu
- `Cmd+Shift+Z` — undo last AI change

## Notes

- App requires Accessibility permissions (for simulating keystrokes via osascript)
- Clipboard-based: copies text, sends to AI, pastes result back
- Select-all uses `Cmd+Up` then `Cmd+Shift+Down` to avoid menu bar flash
- The `assets/` dir in repo root has the original Electron-era icons; Tauri uses `src-tauri/icons/`
