import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface FennecConfig {
  provider: string;
  apiKey: string;
  endpoint: string;
  model: string;
  openaiApiKey: string;
  openaiModel: string;
  shortcuts: {
    correct: string;
    correctAll: string;
    menu: string;
    menuAll: string;
    undo: string;
  };
  launchAtLogin: boolean;
  customActions: { id: string; label: string; subtitle: string; prompt: string; shortcut?: string }[];
}

const $ = (id: string) => document.getElementById(id) as HTMLInputElement;
const $el = (id: string) => document.getElementById(id) as HTMLElement;

// Tauri token → macOS symbol
const KEY_SYMBOLS: Record<string, string> = {
  CmdOrCtrl: "\u2318",
  CommandOrControl: "\u2318",
  Cmd: "\u2318",
  Command: "\u2318",
  Shift: "\u21E7",
  Alt: "\u2325",
  Option: "\u2325",
  Ctrl: "\u2303",
  Control: "\u2303",
};

function renderKeys(raw: string, containerId: string) {
  const container = $el(containerId);
  if (!container) return;
  container.innerHTML = "";
  raw.split("+").forEach((part) => {
    const sym = KEY_SYMBOLS[part];
    const el = document.createElement("span");
    el.className = "key" + (!sym && part.length > 1 ? " key-wide" : "");
    el.textContent = sym || part;
    container.appendChild(el);
  });
}

// Current shortcut values (mutable, written back on save)
let shortcuts: Record<string, string> = {};
let selectedProvider = "radicalbit";

function selectProvider(provider: string) {
  selectedProvider = provider;
  document.querySelectorAll<HTMLElement>(".provider-list .accordion").forEach((acc) => {
    acc.classList.toggle("selected", acc.dataset.provider === provider);
  });
}

// Shortcut recording state
let recordingRow: HTMLElement | null = null;
let recordingKey: string | null = null;

async function startRecording(row: HTMLElement, key: string) {
  stopRecording(false);
  await invoke("pause_shortcuts");
  recordingRow = row;
  recordingKey = key;
  row.classList.add("recording");
}

async function stopRecording(resume = true) {
  if (recordingRow) {
    recordingRow.classList.remove("recording");
  }
  recordingRow = null;
  recordingKey = null;
  if (resume) {
    await invoke("resume_shortcuts");
  }
}

function handleShortcutKeydown(e: KeyboardEvent) {
  if (!recordingRow || !recordingKey) return;

  e.preventDefault();
  e.stopPropagation();

  // Ignore bare modifier presses
  if (["Meta", "Control", "Shift", "Alt"].includes(e.key)) return;

  // Build Tauri shortcut string from held modifiers
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push("CmdOrCtrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");

  // Need at least one modifier
  if (parts.length === 0) {
    stopRecording();
    return;
  }

  // Map the final key
  let finalKey = e.key;
  // Normalize single-char keys to uppercase
  if (finalKey.length === 1) {
    finalKey = finalKey.toUpperCase();
  }
  // Map special keys
  const specialMap: Record<string, string> = {
    ".": ".",
    ",": ",",
    ";": ";",
    "'": "'",
    "/": "/",
    "\\": "\\",
    "[": "[",
    "]": "]",
    "=": "=",
    "-": "-",
    "`": "`",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Escape: "Escape",
    Enter: "Enter",
    Backspace: "Backspace",
    Delete: "Delete",
    Tab: "Tab",
    Space: "Space",
  };
  if (specialMap[e.key]) {
    finalKey = specialMap[e.key];
  }

  parts.push(finalKey);
  const newShortcut = parts.join("+");

  // Update state + render
  shortcuts[recordingKey] = newShortcut;
  renderKeys(newShortcut, recordingKey + "Keys");
  stopRecording();
}

async function init() {
  const config = await invoke<FennecConfig>("get_config");
  console.log("[settings] config loaded:", JSON.stringify(config));

  // Populate connection fields
  $("rbApiKey").value = config.apiKey;
  $("rbEndpoint").value = config.endpoint;
  $("rbModel").value = config.model;
  $("oaiApiKey").value = config.openaiApiKey || "";
  $("oaiModel").value = config.openaiModel || "";
  selectProvider(config.provider || "radicalbit");

  // Store shortcuts
  shortcuts = { ...config.shortcuts };

  // Render all keycaps
  for (const key of ["correct", "correctAll", "menu", "menuAll", "undo"]) {
    renderKeys((shortcuts as any)[key], key + "Keys");
  }

  // ── Tabs ──
  const tabs = document.querySelectorAll<HTMLButtonElement>(".tab");
  const panels = document.querySelectorAll<HTMLElement>(".panel");

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
      stopRecording();
      tabs.forEach((t) => t.classList.remove("active"));
      panels.forEach((p) => p.classList.remove("active"));
      tab.classList.add("active");
      $el("panel-" + tab.dataset.tab!).classList.add("active");
    });
  });

  // ── Accordion + provider selection ──
  document.querySelectorAll<HTMLElement>(".accordion-header").forEach((header) => {
    header.addEventListener("click", (e) => {
      const acc = header.closest(".accordion")!;
      // Click on radio → select provider
      const radio = (e.target as HTMLElement).closest(".provider-radio");
      if (radio) {
        const provider = (acc as HTMLElement).dataset.provider;
        if (provider) {
          selectProvider(provider);
          return;
        }
      }
      // Click elsewhere on header → toggle open/close
      acc.classList.toggle("open");
      header.setAttribute("aria-expanded", acc.classList.contains("open").toString());
    });
  });

  // ── API key visibility toggles ──
  document.querySelectorAll<HTMLButtonElement>(".eye-toggle").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      const targetId = btn.dataset.toggle;
      if (!targetId) return;
      const input = $(targetId);
      input.type = input.type === "password" ? "text" : "password";
    });
  });

  // ── Shortcut recording ──
  document.querySelectorAll<HTMLElement>(".shortcut-row").forEach((row) => {
    row.addEventListener("click", () => {
      const key = row.dataset.shortcut!;
      if (recordingRow === row) {
        stopRecording();
      } else {
        startRecording(row, key);
      }
    });
  });

  document.addEventListener("keydown", (e) => {
    // If recording a shortcut, capture the keypress
    if (recordingRow) {
      handleShortcutKeydown(e);
      return;
    }

    if (e.key === "Escape") {
      getCurrentWindow().close();
    }
  });

  // Click outside a recording row cancels it
  document.addEventListener("click", (e) => {
    if (recordingRow && !(e.target as HTMLElement).closest(".shortcut-row")) {
      stopRecording();
    }
  });

  // ── Save ──
  $el("saveBtn").addEventListener("click", async () => {
    const updated: FennecConfig = {
      ...config,
      provider: selectedProvider,
      apiKey: $("rbApiKey").value,
      endpoint: $("rbEndpoint").value,
      model: $("rbModel").value,
      openaiApiKey: $("oaiApiKey").value,
      openaiModel: $("oaiModel").value,
      shortcuts: {
        correct: shortcuts.correct,
        correctAll: shortcuts.correctAll,
        menu: shortcuts.menu,
        menuAll: shortcuts.menuAll,
        undo: shortcuts.undo,
      },
    };

    await invoke("update_config", { newConfig: updated });
    getCurrentWindow().close();
  });
}

init();
