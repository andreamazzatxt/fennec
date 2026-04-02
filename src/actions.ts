import type { CustomAction } from "./config";

export interface Action {
  label: string;
  subtitle: string;
  icon: string;
  category: "rewrite" | "tone" | "clarity" | "custom";
  buildPrompt: (text: string) => string;
}

export const builtinActions: Record<string, Action> = {
  correct: {
    label: "Smooth it out",
    subtitle: "Clean up spelling & flow",
    icon: "✦",
    category: "rewrite",
    buildPrompt: (text: string) =>
      `Fix any grammar, spelling, and punctuation errors in the following text. ` +
      `Auto-detect the language and reply in the same language. ` +
      `Return ONLY the corrected text, nothing else.\n\n${text}`,
  },
  formal: {
    label: "More polished",
    subtitle: "Professional, composed tone",
    icon: "◆",
    category: "tone",
    buildPrompt: (text: string) =>
      `Rewrite the following text in a formal, professional tone. ` +
      `Auto-detect the language and reply in the same language. ` +
      `Return ONLY the rewritten text, nothing else.\n\n${text}`,
  },
  informal: {
    label: "Keep it casual",
    subtitle: "Warm, natural voice",
    icon: "○",
    category: "tone",
    buildPrompt: (text: string) =>
      `Rewrite the following text in a casual, friendly tone. ` +
      `Auto-detect the language and reply in the same language. ` +
      `Return ONLY the rewritten text, nothing else.\n\n${text}`,
  },
  concise: {
    label: "Say less",
    subtitle: "Trim without losing meaning",
    icon: "▬",
    category: "clarity",
    buildPrompt: (text: string) =>
      `Make the following text more concise while keeping its meaning. ` +
      `Auto-detect the language and reply in the same language. ` +
      `Return ONLY the rewritten text, nothing else.\n\n${text}`,
  },
};

export function getAllActions(customActions: CustomAction[]): Record<string, Action> {
  const all: Record<string, Action> = { ...builtinActions };

  for (const ca of customActions) {
    all[`custom_${ca.id}`] = {
      label: ca.label,
      subtitle: ca.subtitle,
      icon: "◇",
      category: "custom",
      buildPrompt: (text: string) =>
        `${ca.prompt}\n\nAuto-detect the language and reply in the same language. ` +
        `Return ONLY the rewritten text, nothing else.\n\n${text}`,
    };
  }

  return all;
}

export type ActionId = string;
