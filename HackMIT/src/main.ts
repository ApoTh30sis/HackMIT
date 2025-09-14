const activeButtons: Record<string, boolean> = {};
const sliders = document.querySelectorAll<HTMLInputElement>(".volumeSlider");


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


});
