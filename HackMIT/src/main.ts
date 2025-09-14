import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const activeButtons: Record<string, boolean> = {};
let genBtnEl: HTMLButtonElement | null;
let statusEl: HTMLElement | null;
let audioEl: HTMLAudioElement | null;

const getButtonText = (button: HTMLButtonElement, active: boolean): string => {
    if (active) {
        button.classList.add('pressed-button');
    } else {
        button.classList.remove('pressed-button');
    }
    return button.getAttribute(active ? "activeText" : "nonActiveText") ?? button.getAttribute("nonActiveText")!;
};

window.addEventListener("DOMContentLoaded", () => {
    const hiddenButton = document.getElementsByClassName("hidden")[0] as HTMLButtonElement;
    let hidden = true;
    let instrumental = true;
    const all = Array.from(document.querySelectorAll<HTMLButtonElement>(".main-button-style"));
    const buttons = all.filter((b) => b.id !== "generate-btn");

    buttons.forEach((button, idx) => {
        // Initialize button state
        const key = button.id || `btn-${idx}`;
        (button as any).dataset.key = key;
        activeButtons[key] = false;
        // Only set text based on attributes if they exist
        const hasTextAttrs = button.hasAttribute("activeText") || button.hasAttribute("nonActiveText");
        if (hasTextAttrs) {
            button.textContent = getButtonText(button, false);
        }

        // Toggle state on click
        button.addEventListener("click", () => {
            if (button.textContent.indexOf("Instrumental") != -1) {
                if (instrumental) {
                    hiddenButton.classList.add("main-button-style");
                    hiddenButton.classList.remove("hidden");
                } else {
                    hiddenButton.classList.add("hidden");
                    hiddenButton.classList.remove("main-button-style")
                }
                instrumental = ! instrumental;
            }
            const key = (button as any).dataset.key as string;
            const currentState = activeButtons[key];
            const newState = !currentState;

            activeButtons[key] = newState;
            const hasTextAttrs = button.hasAttribute("activeText") || button.hasAttribute("nonActiveText");
            if (hasTextAttrs) {
                button.textContent = getButtonText(button, newState);
            }

            console.log(`${button.id}: ${newState}`);
        });
    });

    hiddenButton.addEventListener("click", () => {
    if (! instrumental) {
        
        if (hidden){
            hiddenButton.textContent = getButtonText(hiddenButton, hidden);
        }
        else{

            hiddenButton.textContent = getButtonText(hiddenButton,hidden);
        }
        hidden = !hidden
    }

        

    });

    const sliders = document.getElementsByClassName("volumeSlider");

    for (let slider of sliders) {
        const input = slider as HTMLInputElement;

        // Find the corresponding span inside the same wrapper
        const wrapper = input.closest(".slider-wrapper") as HTMLElement;
        const valueSpan = wrapper?.querySelector(".sliderValue") as HTMLSpanElement;

        if (valueSpan) {
            // Initialize display
            valueSpan.textContent = (Number(input.value) / 100).toFixed(2);

            // Update on input
            input.addEventListener("input", () => {
                valueSpan.textContent = (Number(input.value) / 100).toFixed(2);
            });
        }
    }
  genBtnEl = document.querySelector("#generate-btn");
  statusEl = document.querySelector("#status");
  audioEl = document.querySelector("#player");

    function collectPreferences() {
        // Genres: read checked boxes in #dropdownList
        const list = document.getElementById("dropdownList");
        const genres: string[] = [];
        if (list) {
            const checks = list.querySelectorAll<HTMLInputElement>('input[type="checkbox"]:checked');
            checks.forEach((c) => genres.push(c.value));
        }
        // Buttons: using their text content to infer state
        // Assume 1st main-button is vocal gender toggle (Male/Female)
        // 2nd is Instrumental On/Off
        const buttons = document.querySelectorAll<HTMLButtonElement>(".main-button-style");
        let vocals_gender: string | null = null;
        let instrumental = true;
        if (buttons[0]) {
            const t = buttons[0].textContent?.toLowerCase() || "";
            vocals_gender = t.includes("female") ? "female" : "male";
        }
        if (buttons[1]) {
            const t = buttons[1].textContent?.toLowerCase() || "";
            instrumental = t.includes("on");
        }
        // Silly button
        const sillyBtn = document.querySelector<HTMLButtonElement>(".silly_button");
        const silly_mode = sillyBtn ? sillyBtn.textContent?.toLowerCase().includes("silly") : false;
        return { genres, vocals_gender, instrumental, silly_mode };
    }

    genBtnEl?.addEventListener("click", async () => {
    if (!statusEl || !audioEl) return;
    statusEl.textContent = "Requesting generation (HackMIT flow)…";
    genBtnEl!.disabled = true;
    try {
            const prefs = collectPreferences();
            const url = await invoke<string>("suno_hackmit_generate_and_wait_with_prefs", { prefs });
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

    // Optional: listen to backend context decisions as before (kept if needed)
        listen("context:decision", async () => {
            // No-op here unless you want to reflect in UI
        });
});
