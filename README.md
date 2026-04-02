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

- **Instant correction** — Select text → press shortcut → corrected text replaces the selection
- **Multiple actions** — Correct, make formal, make casual, make concise
- **Custom actions** — Create your own (e.g. "Translate to English", "Summarize")
- **Works everywhere** — Slack, Mail, Notes, VS Code, browsers, any app
- **Select all + correct** — Fix an entire text field with one shortcut
- **Undo** — Restore the original text if you don't like the result
- **Auto-detect language** — Works in any language, responds in the same language
- **Sound feedback** — A subtle sound when the AI finishes
- **Auto-update** — Stays up to date automatically
- **Launch at login** — Optional, toggle from the tray menu
- **Tiny footprint** — ~8MB app size, powered by Tauri v2

## Install

### Download

Grab the latest `.dmg` from [Releases](https://github.com/andreamazzatxt/fennec/releases), open it, and drag Fennec to Applications.

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

## Setup

On first launch, Fennec opens Settings automatically. You need to configure your AI provider:

1. Go to the **AI Providers** tab
2. Open **Radicalbit AI Gateway**
3. Enter your **API Key**, **Endpoint**, and **Model**
4. Click **Save**

Config is stored locally in `~/.fennec.json`. Your API key never leaves your machine except to call the AI endpoint.

## Shortcuts

| Action | Default | Description |
|--------|---------|-------------|
| Smooth it out | `⌘⇧.` | Correct selected text |
| Smooth all text | `⌘⇧;` | Select all + correct |
| More options | `⌘⇧L` | Open action menu for selection |
| More options for all | `⌘⇧'` | Select all + open action menu |
| Undo last | `⌘⇧Z` | Restore original text |

All shortcuts are customizable in **Settings → Shortcuts**.

## Custom Actions

Create your own text transformations in **Settings → Custom Actions**:

1. Click **+ Add action**
2. Give it a name and description
3. Write a prompt (e.g. *"Translate the following text to English"*)
4. Save — it appears in the options menu

## Development

```bash
bun install
bun run tauri dev    # dev mode with hot reload
bun run tauri build  # build .dmg
```

## Tech

- [Tauri v2](https://tauri.app/) — native macOS app (~8MB vs ~700MB with Electron)
- [Rust](https://www.rust-lang.org/) — backend (AI client, clipboard, config)
- [TypeScript](https://www.typescriptlang.org/) — frontend (shortcuts, notifications)
- [Vite](https://vite.dev/) — build tooling

## How it works

1. You press a shortcut
2. Fennec simulates `⌘C` to copy your selected text
3. Sends it to the AI gateway for processing
4. Simulates `⌘V` to paste the improved text back
5. Restores your original clipboard

The app requires Accessibility permissions to simulate keyboard shortcuts in other apps.

---

<p align="center">
  <sub>Built with care by the Radicalbit team.</sub>
</p>
