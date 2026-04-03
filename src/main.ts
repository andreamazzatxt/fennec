import { sendNotification } from "@tauri-apps/plugin-notification";
import { listen } from "@tauri-apps/api/event";

async function init() {
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

init();
