import type { FennecConfig } from "./config";

async function callAI(config: FennecConfig, prompt: string): Promise<string> {
  const response = await fetch(config.endpoint, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${config.apiKey}`,
    },
    body: JSON.stringify({
      model: config.model,
      messages: [{ role: "user", content: prompt }],
      temperature: 0.3,
    }),
  });

  if (!response.ok) {
    const body = await response.text();
    throw new Error(`AI Gateway error ${response.status}: ${body}`);
  }

  const data = (await response.json()) as {
    choices: { message: { content: string } }[];
  };
  return data.choices[0].message.content.trim();
}

export async function correctText(
  config: FennecConfig,
  prompt: string,
  retries = 1
): Promise<string> {
  try {
    return await callAI(config, prompt);
  } catch (err) {
    const msg = err instanceof Error ? err.message : "";
    // Don't retry guardrail blocks
    if (retries > 0 && !msg.includes("BLOCKED") && !msg.includes("guardrail")) {
      console.log("[fennec] Retrying AI request...");
      await new Promise((r) => setTimeout(r, 500));
      return correctText(config, prompt, retries - 1);
    }
    throw err;
  }
}
