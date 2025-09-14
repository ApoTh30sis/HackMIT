document.addEventListener("DOMContentLoaded", () => {
  const box = document.getElementById("dropdownBox");
  const list = document.getElementById("dropdownList");
  if (!box || !list) return;

  const close = () => { (list as HTMLElement).style.display = "none"; };
  const open = () => { (list as HTMLElement).style.display = "block"; };

  let opened = false;
  box.addEventListener("click", (e) => {
    e.stopPropagation();
    opened ? close() : open();
    opened = !opened;
  });
  document.addEventListener("click", () => {
    if (opened) { close(); opened = false; }
  });
});
document.addEventListener("DOMContentLoaded", () => {
    const dropdownBox = document.getElementById('dropdownBox') as HTMLDivElement;
    const dropdownList = document.getElementById('dropdownList') as HTMLDivElement;
    const checkboxes = dropdownList.querySelectorAll<HTMLInputElement>('input[type="checkbox"]');

    // Toggle dropdown
    dropdownBox.addEventListener('click', () => {
        dropdownList.style.display =
            dropdownList.style.display === 'block' ? 'none' : 'block';
    });

    // Update display text when checkboxes change
    checkboxes.forEach(cb => {
        cb.addEventListener('change', () => {
            const selected = Array.from(checkboxes)
                .filter(c => c.checked)
                .map(c => c.value);

            dropdownBox.textContent =
                selected.length > 0
                    ? `${selected.length} genres selected`
                    : 'Select genres...';
        });
    });

    // Close dropdown if clicked outside
    document.addEventListener('click', (e) => {
        if (!e.target || !(e.target as HTMLElement).closest('.dropdown-container')) {
            dropdownList.style.display = 'none';
        }
    });
});
