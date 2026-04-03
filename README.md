<p align="center">
  <img src="src-tauri/icons/fennec_clean.png" width="120" alt="Fennec">
</p>

<h1 align="center">Fennec</h1>

<p align="center">
  <strong>Your quiet co-writer, living in the menu bar.</strong><br>
  AI-powered text improvement for any app on macOS.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-macOS-black" alt="macOS">
  <img src="https://img.shields.io/badge/tauri-v2-orange" alt="Tauri v2">
  <img src="https://img.shields.io/badge/size-~8MB-green" alt="~8MB">
  <img src="https://img.shields.io/github/v/release/andreamazzatxt/fennec?label=version" alt="Version">
</p>

---

## What is Fennec?

Fennec is a lightweight macOS menu bar app that improves your writing with AI — in **any** text field, in **any** app. Select text, hit a shortcut, and Fennec smooths it out for you.

No windows to manage. No copy-pasting into a chat. Just better text, right where you're typing.

## Features

- **Instant correction** — Select text, press shortcut, corrected text replaces the selection
- **Action menu** — Native macOS popup with actions: Smooth, Formal, Casual, Concise
- **Custom actions** — Create your own actions with custom prompts and emoji icons
- **Works everywhere** — Slack, WhatsApp Web, Mail, Notes, VS Code, browsers, any app
- **Select all + correct** — Fix an entire text field with one shortcut
- **Undo** — Restore the original text if you don't like the result
- **Auto-detect language** — Works in any language, responds in the same language
- **Accessibility API** — Uses AX APIs for direct text read/write, clipboard fallback for web apps
- **Sound feedback** — A subtle sound when the AI finishes
- **Auto-update** — Check for updates and install from within the app
- **Launch at login** — Toggle from the tray menu
- **Menu bar only** — No dock icon, runs purely from the menu bar
- **Tiny footprint** — ~8MB app size, powered by Tauri v2

## Install

### One-liner (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/andreamazzatxt/fennec/main/scripts/install.sh | bash
```

This downloads the latest release, installs it to `/Applications`, and opens the app.

### Download manually

Grab the latest `.dmg` from [Releases](https://github.com/andreamazzatxt/fennec/releases).

> **Note:** Since Fennec is not notarized with Apple, macOS Gatekeeper may block the app when downloaded from a browser. If you see "Fennec is damaged and can't be opened", use the one-liner install above or run:
> ```bash
> xattr -d com.apple.quarantine /Applications/Fennec.app
> ```

### Build from source

```bash
git clone https://github.com/andreamazzatxt/fennec.git
cd fennec
bun install
bun run tauri build
```

The `.dmg` will be in `src-tauri/target/release/bundle/dmg/`.

### Prerequisites (for building)

- [Bun](https://bun.sh/) — JavaScript runtime
- [Rust](https://rustup.rs/) — for the Tauri backend
- Node.js 22+ (via [nvm](https://github.com/nvm-sh/nvm))

## Setup

On first launch, configure your AI provider:

1. Click the Fennec icon in the menu bar and select **Settings...**
2. Go to the **Connection** tab
3. Open the **AI Gateway** accordion (Radical Bit)
4. Enter your **API Key**, **Endpoint**, and **Model**
5. Click **Save**

Config is stored locally in `~/.fennec.json`. Your API key never leaves your machine except to call the AI endpoint.

## Shortcuts

| Action | Default | Description |
|--------|---------|-------------|
| Smooth it out | `⌘⇧.` | Correct selected text |
| Smooth everything | `⌘⇧,` | Select all + correct |
| Pick an action | `⌘⇧L` | Open native action menu for selection |
| Action on everything | `⌘⇧'` | Select all + open action menu |
| Step back | `⌘⇧Z` | Restore original text |

All shortcuts are customizable in **Settings > Shortcuts** — click any shortcut to record a new key combination.

### Action menu

The action menu is a native macOS popup that appears at your cursor with:

- ✨ **Smooth it out** — Fix grammar, spelling, and flow
- 👔 **More formal** — Rewrite in a professional tone
- 😎 **More casual** — Rewrite in a friendly tone
- ✂️ **Make it shorter** — Condense while keeping the meaning
- Plus any **custom actions** you've created

### Custom actions

Create your own actions in **Settings > Actions**:

1. Click **+ Add action**
2. Give it a name, prompt, and pick an emoji icon
3. Save — your action appears in the action menu

## Settings

| Tab | Description |
|-----|-------------|
| **Shortcuts** | Customize all keyboard shortcuts |
| **Connection** | Configure AI provider (AI Gateway or OpenAI) |
| **Actions** | Create and manage custom actions |
| **General** | Check for updates, reset accessibility permissions |
| **Logs** | Real-time debug logs |

## Tray menu

Right-click (or click) the Fennec icon in the menu bar:

- **Fennec vX.Y.Z** — Version display
- **Settings...** — Open the settings window
- **Launch at login** — Toggle autostart
- **Quit Fennec** — Exit the app

## Development

```bash
bun install
bun run tauri dev    # dev mode with hot reload (red tray icon)
bun run tauri build  # build .dmg
```

In dev mode, the tray icon is red to distinguish it from the production app.

## Tech

- [Tauri v2](https://tauri.app/) — native macOS app (~8MB vs ~700MB with Electron)
- [Rust](https://www.rust-lang.org/) — backend (AI client, AX text access, config)
- [TypeScript](https://www.typescriptlang.org/) — frontend (settings UI)
- [Vite 8](https://vite.dev/) — build tooling
- [objc2](https://github.com/madsmtm/objc2) — native NSMenu, NSApplication bindings

## How it works

1. You press a shortcut
2. Fennec reads the selected text via macOS Accessibility APIs (AXSelectedText)
3. Sends it to the AI gateway for processing
4. Writes the improved text back via AX APIs (or clipboard + paste as fallback for web apps)

The app requires Accessibility permissions to read and write text in other apps.

---

<p align="center">
  <sub>Built with care by the Radicalbit team.</sub>
</p>
