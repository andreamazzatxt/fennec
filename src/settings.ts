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
  tapToPolish?: { enabled: boolean; sensitivity: string };
}

const $ = (id: string) => document.getElementById(id) as HTMLInputElement;
const $el = (id: string) => document.getElementById(id) as HTMLElement;

let saveTimeout: ReturnType<typeof setTimeout> | null = null;

function buildConfig(): FennecConfig {
  const tapToggle = $("tapToggle") as HTMLInputElement;
  const tapSensitivity = document.getElementById("tapSensitivity") as HTMLSelectElement;
  return {
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
    launchAtLogin: false,
    customActions: customActions.filter(a => a.label && a.prompt),
    tapToPolish: tapToggle.checked
      ? { enabled: true, sensitivity: tapSensitivity.value }
      : undefined,
  };
}

async function saveNow() {
  await invoke("update_config", { newConfig: buildConfig() });
}

function autoSave() {
  if (saveTimeout) clearTimeout(saveTimeout);
  saveTimeout = setTimeout(async () => {
    try { await saveNow(); } catch (e) { console.error("Auto-save failed:", e); }
  }, 500);
}

function markActionCardDirty(idx: number) {
  const btn = document.querySelector(`[data-save-index="${idx}"]`) as HTMLButtonElement;
  if (btn) btn.disabled = false;
}

function wireSectionSave(btnId: string, inputIds: string[]) {
  const btn = document.getElementById(btnId) as HTMLButtonElement;
  if (!btn) return;
  for (const id of inputIds) {
    $(id).addEventListener("input", () => { btn.disabled = false; });
  }
  btn.addEventListener("click", async () => {
    try {
      await saveNow();
      btn.disabled = true;
      btn.textContent = "Saved";
      setTimeout(() => { btn.textContent = "Save"; }, 1500);
    } catch (e: any) {
      console.error("Save failed:", e);
    }
  });
}

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

function selectProvider(provider: string, save = true) {
  selectedProvider = provider;
  document.querySelectorAll<HTMLElement>(".provider-list .accordion").forEach((acc) => {
    acc.classList.toggle("selected", acc.dataset.provider === provider);
  });
  if (save) {
    // Mark both connection save buttons as dirty since provider changed
    const rbBtn = document.getElementById("saveRb") as HTMLButtonElement;
    const oaiBtn = document.getElementById("saveOai") as HTMLButtonElement;
    if (rbBtn) rbBtn.disabled = false;
    if (oaiBtn) oaiBtn.disabled = false;
  }
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
  autoSave();
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
          <button class="btn-save section-save action-save" data-save-index="${i}" disabled>Save</button>
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
      autoSave();
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
      markActionCardDirty(idx);
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
      markActionCardDirty(idx);
    });
  });

  // Per-action save buttons
  list.querySelectorAll<HTMLButtonElement>(".action-save").forEach(btn => {
    btn.addEventListener("click", async (e) => {
      e.stopPropagation();
      try {
        await saveNow();
        btn.disabled = true;
        btn.textContent = "Saved";
        setTimeout(() => { btn.textContent = "Save"; }, 1500);
      } catch (err) { console.error("Save failed:", err); }
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
  selectProvider(config.provider || "radicalbit", false);

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
    const subtitle = document.querySelector("#resetAxRow .action-subtitle") as HTMLElement;
    btn.disabled = true;
    btn.textContent = "Resetting...";
    try {
      const msg = await invoke<string>("reset_accessibility");
      subtitle.textContent = msg;
      btn.textContent = "Restart";
      btn.classList.remove("btn-action-warn");
      btn.disabled = false;
      btn.onclick = () => { invoke("restart_app"); };
    } catch (e: any) {
      btn.textContent = "Failed";
      subtitle.textContent = `${e}`;
      btn.disabled = false;
    }
  });

  // ── General: Double Tap to Polish ──
  const tapToggle = $("tapToggle") as HTMLInputElement;
  const tapSensitivity = document.getElementById("tapSensitivity") as HTMLSelectElement;
  const tapStatus = $el("tapStatus");

  // Only show on Apple Silicon
  try {
    const isAppleSilicon = await invoke<boolean>("check_tap_hardware");
    if (isAppleSilicon) {
      $el("tapSection").style.display = "";

      if (config.tapToPolish?.enabled) {
        tapToggle.checked = true;
        $el("tapSensitivityRow").style.display = "flex";
        $el("tapHelperRow").style.display = "flex";
        $el("tapTestRow").style.display = "flex";
        tapSensitivity.value = config.tapToPolish.sensitivity || "medium";

        const running = await invoke<boolean>("check_tap_helper_status");
        $el("tapHelperStatus").textContent = running ? "Running" : "Not running";
        if (!running) {
          ($el("tapReinstallBtn") as HTMLElement).style.display = "";
        }
      }
    }
  } catch {}

  tapToggle.addEventListener("change", async () => {
    if (tapToggle.checked) {
      tapStatus.textContent = "Installing helper (admin password required)...";
      $el("tapSensitivityRow").style.display = "flex";
      try {
        await invoke("install_tap_helper", { sensitivity: tapSensitivity.value });
        tapStatus.textContent = "Slap the MacBook to correct all text";
        $el("tapHelperRow").style.display = "flex";
        $el("tapTestRow").style.display = "flex";
        $el("tapHelperStatus").textContent = "Running";
      } catch (e: any) {
        tapToggle.checked = false;
        $el("tapSensitivityRow").style.display = "none";
        tapStatus.textContent = String(e).includes("User canceled")
          ? "Slap the MacBook to correct all text"
          : `Install failed: ${e}`;
      }
    } else {
      try {
        await invoke("uninstall_tap_helper");
      } catch {}
      $el("tapSensitivityRow").style.display = "none";
      $el("tapHelperRow").style.display = "none";
      $el("tapTestRow").style.display = "none";
      tapStatus.textContent = "Slap the MacBook to correct all text";
    }
  });

  $el("tapTestBtn").addEventListener("click", async () => {
    const btn = $el("tapTestBtn") as HTMLButtonElement;
    const status = $el("tapTestStatus");
    btn.disabled = true;
    btn.textContent = "Listening...";
    status.textContent = "Slap the MacBook now...";
    try {
      const msg = await invoke<string>("test_tap_helper");
      status.textContent = msg;
      btn.textContent = "Test";
    } catch (e: any) {
      status.textContent = String(e);
      btn.textContent = "Retry";
    }
    btn.disabled = false;
  });

  $el("tapReinstallBtn").addEventListener("click", async () => {
    const btn = $el("tapReinstallBtn") as HTMLButtonElement;
    btn.disabled = true;
    btn.textContent = "Installing...";
    try {
      await invoke("install_tap_helper", { sensitivity: tapSensitivity.value });
      $el("tapHelperStatus").textContent = "Running";
      btn.style.display = "none";
    } catch (e: any) {
      $el("tapHelperStatus").textContent = `Failed: ${e}`;
    }
    btn.disabled = false;
    btn.textContent = "Reinstall";
  });

  // ── Connection: explicit Save per provider ──
  wireSectionSave("saveRb", ["rbApiKey", "rbEndpoint", "rbModel"]);
  wireSectionSave("saveOai", ["oaiApiKey", "oaiModel"]);

  // ── Other fields: auto-save ──
  tapSensitivity.addEventListener("change", autoSave);
}

init();
