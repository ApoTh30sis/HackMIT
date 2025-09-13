const activeButtons: Record<string, boolean> = {};
const sliders = document.querySelectorAll<HTMLInputElement>(".volumeSlider");

let greetInputEl: HTMLInputElement | null;
let greetMsgEl: HTMLElement | null;
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
    const hiddenButton = document.getElementsByClassName("hidden")[0];
    let hidden = true;
    let instrumental = true;
    const buttons = document.querySelectorAll<HTMLButtonElement>(".main-button-style");

    buttons.forEach((button) => {
        // Initialize button state
        activeButtons[button.id] = false;
        button.textContent = getButtonText(button, false);

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
            const currentState = activeButtons[button.id];
            const newState = !currentState;

            activeButtons[button.id] = newState;
            button.textContent = getButtonText(button, newState);

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
