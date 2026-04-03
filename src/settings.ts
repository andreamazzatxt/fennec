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
  customActions: { id: string; label: string; subtitle: string; prompt: string; shortcut?: string; icon?: string }[];
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
let customActions: { id: string; label: string; subtitle: string; prompt: string; shortcut?: string; icon?: string }[] = [];

async function startRecording(row: HTMLElement, key: string) {
  stopRecording(false);
  await invoke("pause_shortcuts");
  recordingRow = row;
  recordingKey = key;
  row.classList.add("recording");
}

async function stopRecording(resume = true) {
  const wasRecording = recordingRow !== null;
  if (recordingRow) {
    recordingRow.classList.remove("recording");
  }
  recordingRow = null;
  recordingKey = null;
  if (resume && wasRecording) {
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

function renderCustomActions() {
  const list = $el("customActionsList");
  if (customActions.length === 0) {
    list.innerHTML = '<div class="custom-actions-empty">No custom actions yet</div>';
    return;
  }
  list.innerHTML = customActions.map((a, i) => `
    <div class="custom-action-card" data-index="${i}">
      <div class="custom-action-header">
        <button class="icon-picker-btn" data-icon-index="${i}" title="Pick icon">${a.icon || "\u26A1"}</button>
        <span class="custom-action-label">${a.label || "Untitled action"}</span>
        <div class="custom-action-shortcut" id="customShortcut${i}"></div>
        <button class="btn-delete-action" data-delete="${i}" title="Delete">
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
            <path d="M4 4l8 8M12 4l-8 8"/>
          </svg>
        </button>
      </div>
      <div class="custom-action-body">
        <div class="custom-action-fields">
          <div class="field-group">
            <label>Name</label>
            <input type="text" data-field="label" data-index="${i}" value="${a.label}" placeholder="e.g. Translate to English" />
          </div>
          <div class="field-group">
            <label>Prompt</label>
            <textarea data-field="prompt" data-index="${i}" placeholder="e.g. Translate the following text to English">${a.prompt}</textarea>
          </div>
          <div class="field-group">
            <label>Icon</label>
            <div class="emoji-grid" data-emoji-index="${i}">
              ${["✨","🔥","🌍","📝","💬","🎯","🧹","📧","💡","🔬","🎨","📊","🚀","❤️","⚡","🤖"].map(e =>
                `<button class="emoji-option${(a.icon || "⚡") === e ? " selected" : ""}" data-emoji="${e}">${e}</button>`
              ).join("")}
            </div>
          </div>
        </div>
      </div>
    </div>
  `).join("");

  // Render shortcut keycaps
  customActions.forEach((a, i) => {
    if (a.shortcut) renderKeys(a.shortcut, `customShortcut${i}`);
  });

  // Toggle expand on header click
  list.querySelectorAll<HTMLElement>(".custom-action-header").forEach(header => {
    header.addEventListener("click", (e) => {
      if ((e.target as HTMLElement).closest(".btn-delete-action")) return;
      header.closest(".custom-action-card")!.classList.toggle("open");
    });
  });

  // Delete buttons
  list.querySelectorAll<HTMLButtonElement>("[data-delete]").forEach(btn => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      const idx = parseInt(btn.dataset.delete!);
      customActions.splice(idx, 1);
      renderCustomActions();
    });
  });

  // Field changes
  list.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>("[data-field]").forEach(input => {
    input.addEventListener("input", () => {
      const idx = parseInt(input.dataset.index!);
      const field = input.dataset.field as "label" | "prompt";
      customActions[idx][field] = input.value;
      // Update header label live
      if (field === "label") {
        const card = input.closest(".custom-action-card")!;
        card.querySelector(".custom-action-label")!.textContent = input.value || "Untitled action";
      }
    });
  });

  // Emoji selection
  list.querySelectorAll<HTMLElement>(".emoji-grid").forEach(grid => {
    grid.addEventListener("click", (e) => {
      const btn = (e.target as HTMLElement).closest(".emoji-option") as HTMLButtonElement;
      if (!btn) return;
      const idx = parseInt(grid.dataset.emojiIndex!);
      const emoji = btn.dataset.emoji!;
      customActions[idx].icon = emoji;
      // Update selected state
      grid.querySelectorAll(".emoji-option").forEach(b => b.classList.remove("selected"));
      btn.classList.add("selected");
      // Update header icon
      const card = grid.closest(".custom-action-card")!;
      card.querySelector(".icon-picker-btn")!.textContent = emoji;
    });
  });

  // Icon picker button opens card
  list.querySelectorAll<HTMLButtonElement>(".icon-picker-btn").forEach(btn => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      btn.closest(".custom-action-card")!.classList.add("open");
    });
  });
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

  // Load custom actions
  customActions = (config.customActions || []).map(a => ({ ...a }));
  renderCustomActions();

  $el("addActionBtn").addEventListener("click", () => {
    const id = "a" + Date.now().toString(36);
    customActions.push({ id, label: "", subtitle: "", prompt: "" });
    renderCustomActions();
    // Open the new card
    const cards = document.querySelectorAll(".custom-action-card");
    const last = cards[cards.length - 1];
    if (last) {
      last.classList.add("open");
      const input = last.querySelector("input[data-field='label']") as HTMLInputElement;
      if (input) input.focus();
    }
  });

  // Render all keycaps
  for (const key of ["correct", "correctAll", "menu", "menuAll", "undo"]) {
    renderKeys((shortcuts as any)[key], key + "Keys");
  }

  // ── Tabs ──
  const tabs = document.querySelectorAll<HTMLButtonElement>(".tab");
  const panels = document.querySelectorAll<HTMLElement>(".panel");

  let logsInterval: ReturnType<typeof setInterval> | null = null;

  async function loadLogs() {
    const logs = await invoke<string[]>("get_logs");
    const container = $el("logContainer");
    container.textContent = logs.join("\n");
    container.scrollTop = container.scrollHeight;
  }

  tabs.forEach((tab) => {
    tab.addEventListener("click", () => {
      stopRecording();
      tabs.forEach((t) => t.classList.remove("active"));
      panels.forEach((p) => p.classList.remove("active"));
      tab.classList.add("active");
      $el("panel-" + tab.dataset.tab!).classList.add("active");

      // Auto-refresh logs when Logs tab is active
      if (logsInterval) { clearInterval(logsInterval); logsInterval = null; }
      if (tab.dataset.tab === "logs") {
        loadLogs();
        logsInterval = setInterval(loadLogs, 2000);
      }
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

  // ── General: Check for updates ──
  $el("checkUpdateBtn").addEventListener("click", async () => {
    const btn = $el("checkUpdateBtn") as HTMLButtonElement;
    const status = $el("updateStatus");
    btn.disabled = true;
    btn.textContent = "Checking...";
    status.textContent = "Checking for updates...";
    try {
      const version = await invoke<string | null>("check_for_update");
      if (version) {
        status.textContent = `Version ${version} available!`;
        btn.textContent = "Install";
        btn.classList.add("btn-action-success");
        btn.disabled = false;
        btn.onclick = async () => {
          btn.disabled = true;
          btn.textContent = "Installing...";
          status.textContent = "Downloading and installing...";
          try {
            await invoke("install_update");
            status.textContent = "Update installed!";
            btn.textContent = "Restart";
            btn.classList.remove("btn-action-success");
            btn.disabled = false;
            btn.onclick = () => { invoke("restart_app"); };
          } catch (e: any) {
            status.textContent = `Install failed: ${e}`;
            btn.textContent = "Retry";
            btn.disabled = false;
          }
        };
      } else {
        status.textContent = "You're on the latest version";
        btn.textContent = "Check";
        btn.disabled = false;
      }
    } catch (e: any) {
      status.textContent = `Error: ${e}`;
      btn.textContent = "Retry";
      btn.disabled = false;
    }
  });

  // ── General: Reset Accessibility ──
  $el("resetAxBtn").addEventListener("click", async () => {
    const btn = $el("resetAxBtn") as HTMLButtonElement;
    btn.disabled = true;
    btn.textContent = "Resetting...";
    try {
      const msg = await invoke<string>("reset_accessibility");
      btn.textContent = "Done";
      const subtitle = document.querySelector("#resetAxRow .action-subtitle") as HTMLElement;
      subtitle.textContent = msg;
    } catch (e: any) {
      btn.textContent = "Failed";
      const subtitle = document.querySelector("#resetAxRow .action-subtitle") as HTMLElement;
      subtitle.textContent = `${e}`;
      btn.disabled = false;
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
      customActions: customActions.filter(a => a.label && a.prompt),
    };

    try {
      await invoke("update_config", { newConfig: updated });
      getCurrentWindow().close();
    } catch (e: any) {
      console.error("Save failed:", e);
      const status = $el("status");
      status.querySelector("svg")!.style.display = "none";
      status.textContent = `Error: ${e}`;
      status.classList.add("visible");
    }
  });
}

init();
