import {
  app,
  globalShortcut,
  clipboard,
  Tray,
  Menu,
  nativeImage,
  screen,
  BrowserWindow,
  Notification,
  ipcMain,
  shell,
} from "electron";
import { execSync } from "child_process";
import * as path from "path";
import { autoUpdater } from "electron-updater";
import { correctText } from "./ai";
import { loadConfig, saveConfig, type FennecConfig } from "./config";
import { getAllActions, builtinActions, type ActionId } from "./actions";

let tray: Tray | null = null;
let hiddenWindow: BrowserWindow | null = null;
let loaderWindow: BrowserWindow | null = null;
let settingsWindow: BrowserWindow | null = null;
let isProcessing = false;
let pendingText: string | null = null;
let pendingPreviousClipboard: string | null = null;
let currentConfig: FennecConfig;

// Undo history
let lastOriginalText: string | null = null;
let lastReplacedText: string | null = null;

app.dock?.hide();

app.whenReady().then(() => {
  currentConfig = loadConfig();

  hiddenWindow = new BrowserWindow({
    width: 0,
    height: 0,
    show: false,
    skipTaskbar: true,
  });

  createTray();
  setupIPC();
  setupAutoUpdater();

  // First launch: open settings if no API key
  if (!currentConfig.apiKey) {
    openSettings("providers");
    return;
  }

  registerShortcuts();
});

function createTray() {
  const iconPath = path.join(__dirname, "..", "assets", "tray_color.png");
  const icon = nativeImage.createFromPath(iconPath);
  tray = new Tray(icon.resize({ width: 18, height: 18 }));
  tray.setToolTip("Fennec");
  updateTrayMenu();
}

function formatAccelerator(accel: string): string {
  return accel
    .replace("CommandOrControl+", "⌘")
    .replace("Shift+", "⇧")
    .replace("Alt+", "⌥")
    .replace("Control+", "⌃");
}

function updateTrayMenu() {
  const s = currentConfig.shortcuts;
  const contextMenu = Menu.buildFromTemplate([
    { label: isProcessing ? "⏳ Thinking..." : "Fennec", enabled: false },
    { type: "separator" },
    {
      label: `Smooth it out (${formatAccelerator(s.correct)})`,
      click: () => executeAction("correct"),
      enabled: !isProcessing,
    },
    {
      label: `Smooth all text (${formatAccelerator(s.correctAll)})`,
      click: () => executeSelectAllAndCorrect(),
      enabled: !isProcessing,
    },
    {
      label: `More options... (${formatAccelerator(s.menu)})`,
      click: () => showPopupMenu(),
      enabled: !isProcessing,
    },
    {
      label: `More options for all (${formatAccelerator(s.menuAll)})`,
      click: () => showPopupMenuAll(),
      enabled: !isProcessing,
    },
    {
      label: `Undo last (${formatAccelerator(s.undo)})`,
      click: () => undoLast(),
      enabled: !!lastOriginalText,
    },
    { type: "separator" },
    {
      label: "Launch at login",
      type: "checkbox",
      checked: currentConfig.launchAtLogin,
      click: (menuItem) => {
        currentConfig.launchAtLogin = menuItem.checked;
        saveConfig(currentConfig);
        app.setLoginItemSettings({ openAtLogin: menuItem.checked });
      },
    },
    { label: "Settings...", click: () => openSettings() },
    { label: "Quit", click: () => app.quit() },
  ]);
  tray?.setContextMenu(contextMenu);
}

function registerShortcuts() {
  globalShortcut.unregisterAll();
  const s = currentConfig.shortcuts;

  const register = (accel: string, fn: () => void) => {
    try {
      const ok = globalShortcut.register(accel, fn);
      console.log(`[fennec] ${accel} registered: ${ok}`);
    } catch (err) {
      console.error(`[fennec] Failed to register ${accel}:`, err);
    }
  };

  register(s.correct, () => executeAction("correct"));
  register(s.menu, () => showPopupMenu());
  register(s.correctAll, () => executeSelectAllAndCorrect());
  register(s.menuAll, () => showPopupMenuAll());
  register(s.undo, () => undoLast());
}

function setupIPC() {
  ipcMain.on("save-config", (_event, config) => {
    const prevLaunch = currentConfig.launchAtLogin;
    currentConfig = {
      ...currentConfig,
      ...config,
      shortcuts: { ...currentConfig.shortcuts, ...config.shortcuts },
      customActions: config.customActions || currentConfig.customActions,
    };
    saveConfig(currentConfig);

    if (currentConfig.launchAtLogin !== prevLaunch) {
      app.setLoginItemSettings({ openAtLogin: currentConfig.launchAtLogin });
    }

    registerShortcuts();
    updateTrayMenu();
    settingsWindow?.close();
    settingsWindow = null;
    console.log("[fennec] Config saved");
  });

  ipcMain.on("close-settings", () => {
    settingsWindow?.close();
    settingsWindow = null;
  });
}

function openSettings(tab?: string) {
  if (settingsWindow) {
    settingsWindow.focus();
    return;
  }

  globalShortcut.unregisterAll();

  app.dock?.setIcon(
    nativeImage.createFromPath(path.join(__dirname, "..", "assets", "fennec_clean.png"))
  );
  app.dock?.show();

  settingsWindow = new BrowserWindow({
    width: 480,
    height: 520,
    resizable: false,
    title: "Fennec Settings",
    titleBarStyle: "hiddenInset",
    backgroundColor: "#FAF7F0",
    show: false,
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false,
    },
  });

  settingsWindow.loadFile(path.join(__dirname, "..", "src", "settings.html"));

  settingsWindow.once("ready-to-show", () => {
    settingsWindow?.show();
    settingsWindow?.webContents.send("load-config", currentConfig, tab);
  });

  settingsWindow.on("closed", () => {
    settingsWindow = null;
    app.dock?.hide();
    if (currentConfig.apiKey) {
      registerShortcuts();
    }
  });
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// --- Sound feedback ---

function playDoneSound() {
  try {
    execSync(`afplay /System/Library/Sounds/Tink.aiff &`);
  } catch {
    // Ignore sound errors
  }
}

// --- Undo ---

async function undoLast() {
  if (!lastOriginalText) {
    new Notification({ title: "Fennec", body: "Nothing to undo." }).show();
    return;
  }

  clipboard.writeText(lastOriginalText);

  execSync(
    `osascript -e 'tell application "System Events" to keystroke "v" using command down'`
  );

  await sleep(300);
  const restored = lastOriginalText;
  lastOriginalText = null;
  lastReplacedText = null;
  clipboard.writeText("");
  updateTrayMenu();
  console.log(`[fennec] Undone to: "${restored.substring(0, 50)}..."`);
}

// --- Copy helper ---

async function copySelectedText(): Promise<string | null> {
  await sleep(300);

  try {
    execSync(
      `osascript -e 'tell application "System Events" to keystroke "c" using command down'`
    );
  } catch (err) {
    console.error("[fennec] Copy failed:", err);
    return null;
  }

  await sleep(200);

  const text = clipboard.readText();
  if (!text.trim()) {
    return null;
  }
  return text;
}

// --- Loader ---

function showLoader() {
  const cursor = screen.getCursorScreenPoint();
  loaderWindow = new BrowserWindow({
    width: 160,
    height: 36,
    x: cursor.x + 12,
    y: cursor.y + 12,
    frame: false,
    transparent: true,
    alwaysOnTop: true,
    skipTaskbar: true,
    resizable: false,
    hasShadow: false,
    roundedCorners: true,
    focusable: false,
    show: false,
    webPreferences: { nodeIntegration: false },
  });
  loaderWindow.setIgnoreMouseEvents(true);
  loaderWindow.loadFile(path.join(__dirname, "..", "src", "loader.html"));
  loaderWindow.showInactive();
}

function hideLoader() {
  if (loaderWindow) {
    loaderWindow.close();
    loaderWindow = null;
  }
}

// --- Build popup menu items ---

function buildMenuItems() {
  const allActions = getAllActions(currentConfig.customActions);
  return Object.entries(allActions).map(([id, action]) => {
    const iconPath = path.join(__dirname, "..", "assets", `icon_${id}.png`);
    let icon: Electron.NativeImage | undefined;
    try {
      const img = nativeImage.createFromPath(iconPath);
      if (!img.isEmpty()) {
        icon = img.resize({ width: 16, height: 16 });
      }
    } catch {
      // No icon for custom actions
    }
    return {
      id,
      label: `${action.label}  —  ${action.subtitle}`,
      icon,
      click: () => {
        executeActionWithText(id as ActionId, pendingText!, pendingPreviousClipboard!);
        pendingText = null;
        pendingPreviousClipboard = null;
      },
    };
  });
}

// --- Popup Menu (native) ---

async function showPopupMenu() {
  pendingPreviousClipboard = clipboard.readText();
  pendingText = await copySelectedText();

  if (!pendingText) {
    console.log("[fennec] No text selected, skipping popup");
    pendingText = null;
    pendingPreviousClipboard = null;
    return;
  }

  console.log(`[fennec] Pre-copied for popup: "${pendingText.substring(0, 50)}..."`);

  const menu = Menu.buildFromTemplate(buildMenuItems());
  menu.popup({ window: hiddenWindow! });
}

async function showPopupMenuAll() {
  if (isProcessing) return;

  pendingPreviousClipboard = clipboard.readText();
  await sleep(300);

  try {
    execSync(
      `osascript -e 'tell application "System Events" to key code 126 using command down' -e 'delay 0.05' -e 'tell application "System Events" to key code 125 using {command down, shift down}'`
    );
  } catch (err) {
    console.error("[fennec] Select all failed:", err);
    return;
  }

  await sleep(150);
  pendingText = await copySelectedText();

  if (!pendingText) {
    console.log("[fennec] No text found, skipping popup");
    pendingText = null;
    pendingPreviousClipboard = null;
    return;
  }

  const menu = Menu.buildFromTemplate(buildMenuItems());
  menu.popup({ window: hiddenWindow! });
}

// --- Select All + Correct ---

async function executeSelectAllAndCorrect() {
  if (isProcessing) return;

  const previousClipboard = clipboard.readText();
  await sleep(300);

  try {
    execSync(
      `osascript -e 'tell application "System Events" to key code 126 using command down' -e 'delay 0.05' -e 'tell application "System Events" to key code 125 using {command down, shift down}'`
    );
  } catch (err) {
    console.error("[fennec] Select all failed:", err);
    return;
  }

  await sleep(150);

  try {
    execSync(
      `osascript -e 'tell application "System Events" to keystroke "c" using command down'`
    );
  } catch (err) {
    console.error("[fennec] Copy failed:", err);
    return;
  }

  await sleep(200);

  const selectedText = clipboard.readText();
  if (!selectedText.trim()) {
    console.log("[fennec] No text found, skipping");
    return;
  }

  await executeActionWithText("correct", selectedText, previousClipboard);
}

// --- Execute Action ---

async function executeAction(actionId: ActionId) {
  if (isProcessing) return;

  const previousClipboard = clipboard.readText();
  const selectedText = await copySelectedText();

  if (!selectedText) {
    console.log("[fennec] No text selected, skipping");
    return;
  }

  await executeActionWithText(actionId, selectedText, previousClipboard);
}

async function executeActionWithText(
  actionId: ActionId,
  selectedText: string,
  previousClipboard: string
) {
  if (isProcessing) return;

  const allActions = getAllActions(currentConfig.customActions);
  const action = allActions[actionId];
  if (!action) {
    console.error(`[fennec] Unknown action: ${actionId}`);
    return;
  }

  console.log(`[fennec] Text: "${selectedText.substring(0, 50)}..."`);

  const prompt = action.buildPrompt(selectedText);

  isProcessing = true;
  tray?.setTitle(" ⏳");
  updateTrayMenu();
  showLoader();

  try {
    console.log(`[fennec] Sending to AI (action: ${actionId})...`);
    const result = await correctText(currentConfig, prompt);
    console.log(`[fennec] Got result: "${result.substring(0, 50)}..."`);

    hideLoader();

    // Save for undo
    lastOriginalText = selectedText;
    lastReplacedText = result;

    clipboard.writeText(result);

    execSync(
      `osascript -e 'tell application "System Events" to keystroke "v" using command down'`
    );

    playDoneSound();

    await sleep(300);
    clipboard.writeText(previousClipboard);
  } catch (err) {
    console.error("[fennec] AI request failed:", err);
    hideLoader();
    clipboard.writeText(previousClipboard);

    const errMsg = err instanceof Error ? err.message : String(err);
    if (errMsg.includes("BLOCKED") || errMsg.includes("guardrail")) {
      new Notification({
        title: "Fennec",
        body: "The AI gateway blocked this text. This can happen with typos — try rephrasing slightly.",
      }).show();
    } else {
      new Notification({
        title: "Fennec",
        body: "Something went wrong. Check your connection or API key.",
      }).show();
    }
  } finally {
    isProcessing = false;
    tray?.setTitle("");
    updateTrayMenu();
  }
}

// --- Auto Updater ---

function setupAutoUpdater() {
  if (!app.isPackaged) {
    console.log("[fennec] Skipping auto-update in dev mode");
    return;
  }

  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = true;

  autoUpdater.on("update-available", (info) => {
    console.log(`[fennec] Update available: ${info.version}`);
    new Notification({
      title: "Fennec",
      body: `Version ${info.version} is downloading...`,
    }).show();
  });

  autoUpdater.on("update-downloaded", (info) => {
    console.log(`[fennec] Update downloaded: ${info.version}`);
    new Notification({
      title: "Fennec",
      body: `Version ${info.version} ready. It will install on next restart.`,
    }).show();
  });

  autoUpdater.on("error", (err) => {
    console.error("[fennec] Auto-update error:", err.message);
  });

  // Check for updates every 4 hours
  autoUpdater.checkForUpdates().catch(() => {});
  setInterval(() => {
    autoUpdater.checkForUpdates().catch(() => {});
  }, 4 * 60 * 60 * 1000);
}

app.on("will-quit", () => {
  try {
    globalShortcut.unregisterAll();
  } catch {
    // Ignore
  }
});

app.on("window-all-closed", () => {
  // Keep app running
});
