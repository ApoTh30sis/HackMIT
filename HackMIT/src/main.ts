import { invoke } from "@tauri-apps/api/core";

let greetInputEl: HTMLInputElement | null;
let greetMsgEl: HTMLElement | null;
let genBtnEl: HTMLButtonElement | null;
let statusEl: HTMLElement | null;
let audioEl: HTMLAudioElement | null;

async function greet() {
  if (greetMsgEl && greetInputEl) {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    greetMsgEl.textContent = await invoke("greet", {
      name: greetInputEl.value,
    });
  }
}

window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  document.querySelector("#greet-form")?.addEventListener("submit", (e) => {
    e.preventDefault();
    greet();
  });

  genBtnEl = document.querySelector("#generate-btn");
  statusEl = document.querySelector("#status");
  audioEl = document.querySelector("#player");

  genBtnEl?.addEventListener("click", async () => {
    if (!statusEl || !audioEl) return;
    statusEl.textContent = "Requesting generation (HackMIT flow)…";
    genBtnEl!.disabled = true;
    try {
      const url = await invoke<string>("suno_hackmit_generate_and_wait");
      statusEl.textContent = "Stream ready. Playing…";
      audioEl.src = url;
      await audioEl.play().catch(() => {
        // If autoplay blocked, user can press play
      });
    } catch (err: any) {
      statusEl.textContent = `Error: ${err?.toString?.() ?? "unknown"}`;
    } finally {
      genBtnEl!.disabled = false;
    }
  });
});
