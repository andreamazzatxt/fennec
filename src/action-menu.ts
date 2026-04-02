import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const selectAll = new URLSearchParams(window.location.search).get("selectAll") === "true";

let selectedIndex = 0;
const actions = document.querySelectorAll<HTMLButtonElement>(".action");

function updateSelection() {
  actions.forEach((a, i) => {
    a.classList.toggle("selected", i === selectedIndex);
  });
}

function runAction(actionId: string) {
  // Fire and forget — Rust closes the window, waits for focus, then executes
  invoke("execute_action_deferred", { actionId, selectAll });
}

// Click
actions.forEach((btn) => {
  btn.addEventListener("mousedown", (e) => {
    e.preventDefault();
    runAction(btn.dataset.action!);
  });
});

// Keyboard
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    getCurrentWindow().close();
    return;
  }
  if (e.key === "ArrowDown") {
    e.preventDefault();
    selectedIndex = (selectedIndex + 1) % actions.length;
    updateSelection();
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    selectedIndex = (selectedIndex - 1 + actions.length) % actions.length;
    updateSelection();
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    runAction(actions[selectedIndex].dataset.action!);
  }
});

// Close on blur (delay to avoid triggering on open)
setTimeout(() => {
  getCurrentWindow().onFocusChanged(({ payload: focused }) => {
    if (!focused) getCurrentWindow().close();
  });
}, 500);

updateSelection();
