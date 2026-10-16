// *Copy* buttons: each copies the text of the field its `data-copy` names
// and says so for a moment. They stay hidden without this script.

/** Shows every copy button under `root` and makes it work. */
export function setUpCopy(root: ParentNode): void {
  for (const button of Array.from(root.querySelectorAll<HTMLButtonElement>("button[data-copy]"))) {
    button.hidden = false;
    const label = button.textContent ?? "";
    button.addEventListener("click", () => {
      const field = document.getElementById(button.dataset["copy"] ?? "");
      if (!(field instanceof HTMLTextAreaElement || field instanceof HTMLInputElement)) return;
      field.select();
      const done = (): void => {
        button.textContent = button.dataset["copied"] ?? label;
        window.setTimeout(() => (button.textContent = label), 2000);
      };
      if (navigator.clipboard) navigator.clipboard.writeText(field.value).then(done, () => undefined);
    });
  }
}
