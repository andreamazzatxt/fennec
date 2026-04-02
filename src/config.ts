import * as fs from "fs";
import * as path from "path";
import * as os from "os";

export interface ShortcutConfig {
  correct: string;
  correctAll: string;
  menu: string;
  menuAll: string;
  undo: string;
}

export interface CustomAction {
  id: string;
  label: string;
  subtitle: string;
  prompt: string;
  shortcut?: string;
}

export interface FennecConfig {
  apiKey: string;
  endpoint: string;
  model: string;
  shortcuts: ShortcutConfig;
  launchAtLogin: boolean;
  customActions: CustomAction[];
}

const CONFIG_PATH = path.join(os.homedir(), ".fennec.json");

const DEFAULTS: FennecConfig = {
  apiKey: "",
  endpoint: "https://ai-gateway.radicalbit.ai/v1/chat/completions",
  model: "dev-mattia-ripamonti",
  shortcuts: {
    correct: "CommandOrControl+Shift+.",
    correctAll: "CommandOrControl+Shift+;",
    menu: "CommandOrControl+Shift+L",
    menuAll: "CommandOrControl+Shift+'",
    undo: "CommandOrControl+Shift+Z",
  },
  launchAtLogin: false,
  customActions: [],
};

export function loadConfig(): FennecConfig {
  try {
    const raw = fs.readFileSync(CONFIG_PATH, "utf-8");
    const userConfig = JSON.parse(raw);
    return {
      ...DEFAULTS,
      ...userConfig,
      shortcuts: { ...DEFAULTS.shortcuts, ...userConfig.shortcuts },
      customActions: userConfig.customActions || [],
    };
  } catch {
    return DEFAULTS;
  }
}

export function saveConfig(config: FennecConfig): void {
  fs.writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2), "utf-8");
}
