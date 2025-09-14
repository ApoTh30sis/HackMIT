import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const activeButtons: Record<string, boolean> = {};
let genBtnEl: HTMLButtonElement | null;
let statusEl: HTMLElement | null;
let audioEl: HTMLAudioElement | null;
let nextUrl: string | null = null;
let contextEl: HTMLElement | null;
let generating = false;
let history: string[] = []; // played track URLs (for Back)

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
            if (button.textContent && button.textContent.indexOf("Instrumental") != -1) {
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
    contextEl = document.querySelector("#context");
    const backBtn = document.querySelector<HTMLButtonElement>("#btn-back");
    const playPauseBtn = document.querySelector<HTMLButtonElement>("#btn-play-pause");
    const forwardBtn = document.querySelector<HTMLButtonElement>("#btn-forward");

    function pushLog(_msg: string) {
        // GUI logs disabled per request
        // if (!logEl) return;
        // const t = new Date().toLocaleTimeString();
        // const p = document.createElement("div");
        // p.textContent = `[${t}] ${msg}`;
        // logEl.prepend(p);
    }

    // Touch helpers to avoid tree-shaking/unused warnings in dev
    if (Math.random() < 0) {
        pushLog("");
    }


    async function generateTrack(): Promise<string> {
        const prefs = collectPreferences();
        return await invoke<string>("suno_hackmit_generate_and_wait_with_prefs", { prefs });
    }

    async function fadeOutAndSwitch(newUrl: string) {
        if (!audioEl) return;
    // Save current to history before switching
    if (audioEl.src) { history.push(audioEl.src); }
        const startVol = audioEl.volume;
        const steps = 10;
        const intervalMs = 100;
        for (let i = 0; i < steps; i++) {
            audioEl.volume = Math.max(0, startVol * (1 - (i + 1) / steps));
            await new Promise((r) => setTimeout(r, intervalMs));
        }
        audioEl.pause();
        audioEl.src = newUrl;
        audioEl.volume = startVol;
        try { await audioEl.play(); } catch {}
    }

    function collectPreferences() {
        // Genres: read checked boxes in #dropdownList
        const list = document.getElementById("dropdownList");
        const genres: string[] = [];
        if (list) {
            const checks = list.querySelectorAll<HTMLInputElement>('input[type="checkbox"]:checked');
            checks.forEach((c) => genres.push(c.value));
        }
        // Buttons: infer by label text (robust to order)
        const buttons = document.querySelectorAll<HTMLButtonElement>(".main-button-style");
        let vocals_gender: string | null = null;
        let instrumental = true;
        buttons.forEach((b) => {
            const t = (b.textContent || "").toLowerCase();
            if (t.includes("instrumental")) {
                instrumental = t.includes("on");
            } else if (t.includes("vocals")) {
            vocals_gender = t.includes("female") ? "female" : "male";
        }
        });
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
                const url = await generateTrack();
      statusEl.textContent = "Stream ready. Playing…";
      audioEl.src = url;
      await audioEl.play().catch(() => {
        // If autoplay blocked, user can press play
      });
            // Pre-generate next track so there's no gap at end
                    if (!nextUrl && !generating) {
                generating = true;
                        generateTrack().then((u) => { nextUrl = u; pushLog("prefetched next track"); }).catch(() => {}).finally(() => { generating = false; });
            }
    } catch (err: any) {
      statusEl.textContent = `Error: ${err?.toString?.() ?? "unknown"}`;
    } finally {
      genBtnEl!.disabled = false;
    }
  });

    // Never pause: when current ends, immediately play next or generate one
                audioEl?.addEventListener("ended", async () => {
            if (!audioEl) return;
                    const next = nextUrl;
                    if (next) {
                        const u = next; nextUrl = null;
        if (audioEl.src) history.push(audioEl.src);
                audioEl.src = u;
                try { await audioEl.play(); } catch {}
                // Preload the following one
                    if (!generating) { generating = true; generateTrack().then((nu) => { nextUrl = nu; pushLog("prefetched next after switch"); }).catch(() => {}).finally(() => { generating = false; }); }
            } else {
                    // Ensure there's no pause: restart current track while generating next
                    const resumeVol = audioEl.volume;
                    audioEl.currentTime = 0;
                    audioEl.volume = resumeVol;
                    try { await audioEl.play(); } catch {}
                    if (!generating) {
                        generating = true;
                              generateTrack().then((u) => { nextUrl = u; pushLog("prefetched next after restart"); }).catch(() => {}).finally(() => { generating = false; });
                    }
            }
        });

        // Listen to backend context decisions: switch or queue
            listen("context:decision", async (ev) => {
            const payload: any = (ev as any).payload;
            const action = payload?.action as string | undefined;
            if (!audioEl || !action) return;
            // Show context in UI
            const ctx = payload?.current_context;
            const prev = payload?.previous_context;
            if (contextEl && ctx) {
                const prevTag = prev?.tag ? ` (prev: ${prev.tag})` : "";
                let contextText = `Context: ${ctx.tag} — ${ctx.details}${prevTag}`;
                
                // Add music tags to context display
                try {
                    const musicTags = await invoke<string | null>("get_current_music_tags");
                    if (musicTags) {
                        contextText += ` | Music: ${musicTags}`;
                    }
                } catch (e) {
                    console.log("Could not fetch music tags:", e);
                }
                
                contextEl.textContent = contextText;
            }
            if (action === "switch_with_fade") {
                // High-priority: regenerate JSON with Claude and play asap, preempting queue
                (async () => {
                    try {
                        const url = await generateTrack(); // invokes backend which regenerates suno_request.json from latest screenshot
                        await fadeOutAndSwitch(url);
                        // Optionally warm a next track without blocking
                        generateTrack().then((nu) => { nextUrl = nu; pushLog("prefetched next after fade switch"); }).catch(() => {});
                    } catch (e) {
                        pushLog(`priority generation failed: ${e}`);
                    }
                })();
            } else {
                // continue: ensure we have a next track ready
                if (!nextUrl && !generating) {
                    generating = true;
                          generateTrack().then((u) => { nextUrl = u; pushLog("prefetched next on continue"); }).catch(() => {}).finally(() => { generating = false; });
                }
            }
        });


            // Listen for music:switch events from backend - immediately switch to new music
            listen("music:switch", async (ev) => {
                const audioUrl = ev.payload as string;
                if (audioUrl && audioEl) {
                    console.log("Received music:switch, immediately switching to:", audioUrl);
                    await fadeOutAndSwitch(audioUrl);
                    
                    // Update context display with new music tags
                    if (contextEl) {
                        try {
                            const musicTags = await invoke<string | null>("get_current_music_tags");
                            if (musicTags) {
                                const currentText = contextEl.textContent || "";
                                // Update the music part of the context text
                                const baseText = currentText.split(" | Music:")[0];
                                contextEl.textContent = `${baseText} | Music: ${musicTags}`;
                            }
                        } catch (e) {
                            console.log("Could not fetch music tags after switch:", e);
                        }
                    }
                }
            });

            // Listen for music:error events from backend
            listen("music:error", async (ev) => {
                const errorMsg = ev.payload as string;
                if (statusEl) {
                    statusEl.textContent = `Error: ${errorMsg}`;
                }
                console.error("Music generation error:", errorMsg);
            });

            // Gray-out vocals when instrumental is ON (robust to order)
            const mainButtons = document.querySelectorAll<HTMLButtonElement>(".main-button-style");
            let vocalsBtn: HTMLButtonElement | undefined;
            let instrumentalBtn: HTMLButtonElement | undefined;
            mainButtons.forEach((b) => {
                const t = (b.textContent || "").toLowerCase();
                if (t.includes("vocals")) vocalsBtn = b;
                if (t.includes("instrumental")) instrumentalBtn = b;
            });
            function applyVocalsDisabled() {
                const t = instrumentalBtn?.textContent?.toLowerCase() || "";
                const on = t.includes("instrumental : on");
                if (vocalsBtn) {
                    vocalsBtn.disabled = on;
                    vocalsBtn.style.opacity = on ? "0.5" : "1";
                    vocalsBtn.style.pointerEvents = on ? "none" : "auto";
                }
            }
            instrumentalBtn?.addEventListener("click", applyVocalsDisabled);
            applyVocalsDisabled();

            // Controls: Back / Play-Pause / Forward
            backBtn?.addEventListener("click", async () => {
                if (!audioEl) return;
                const prev = history.pop();
                if (prev) {
                    try { audioEl.src = prev; await audioEl.play(); } catch {}
                } else {
                    // If no history, restart current
                    try { audioEl.currentTime = 0; await audioEl.play(); } catch {}
                }
            });
        playPauseBtn?.addEventListener("click", async () => {
                if (!audioEl) return;
                if (audioEl.paused) {
            try { await audioEl.play(); playPauseBtn.textContent = "⏸"; } catch {}
                } else {
                    audioEl.pause();
            playPauseBtn.textContent = "▶";
                }
            });
            forwardBtn?.addEventListener("click", async () => {
                if (!audioEl) return;
                const next = nextUrl;
                if (next) {
                    if (audioEl.src) history.push(audioEl.src);
                    try { audioEl.src = next; await audioEl.play(); nextUrl = null; } catch {}
                } else {
                    // If nothing queued, trigger a generation and play when done
                    const url = await generateTrack().catch(() => null);
                    if (url) {
                        if (audioEl.src) history.push(audioEl.src);
                        try { audioEl.src = url; await audioEl.play(); } catch {}
                    }
                }
            });

            // Keep Play/Pause button label in sync with media state
            if (playPauseBtn && audioEl) {
                const syncBtn = () => { playPauseBtn.textContent = audioEl!.paused ? "▶" : "⏸"; };
                audioEl.addEventListener("play", syncBtn);
                audioEl.addEventListener("pause", syncBtn);
                syncBtn();
            }
});
