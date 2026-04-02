import { invoke } from "@tauri-apps/api/core";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import { sendNotification } from "@tauri-apps/plugin-notification";
import { listen } from "@tauri-apps/api/event";

interface FennecConfig {
  apiKey: string;
  endpoint: string;
  model: string;
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

let config: FennecConfig;

async function init() {
  config = await invoke<FennecConfig>("get_config");

  if (!config.apiKey) {
    return;
  }

  await registerShortcuts();

  await listen<string>("fennec:error", (event) => {
    const msg = event.payload;
    if (msg.includes("BLOCKED") || msg.includes("guardrail")) {
      sendNotification({
        title: "Fennec",
        body: "The AI gateway blocked this text. Try rephrasing slightly.",
      });
    } else {
      sendNotification({
        title: "Fennec",
        body: "Something went wrong. Check your connection or API key.",
      });
    }
  });
}

async function registerShortcuts() {
  await unregisterAll();
  const s = config.shortcuts;

  await register(s.correct, async () => {
    await invoke("execute_action", { actionId: "correct", selectAll: false });
  });

  await register(s.correctAll, async () => {
    await invoke("execute_action", { actionId: "correct", selectAll: true });
  });

  await register(s.undo, async () => {
    try {
      await invoke("undo_last");
    } catch {
      sendNotification({ title: "Fennec", body: "Nothing to undo." });
    }
  });

  for (const action of config.customActions) {
    if (action.shortcut) {
      await register(action.shortcut, async () => {
        await invoke("execute_action", { actionId: `custom_${action.id}`, selectAll: false });
      });
    }
  }
}

init();
