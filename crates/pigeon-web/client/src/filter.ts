// The Files page's filter by name (ux.md §6.2): typing hides the rows
// whose name does not contain the words typed, ignoring case and accents,
// and says so when none is left. Without this script the box stays hidden.

/** `text` in lower case without accents, for comparing names. */
export function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{Mn}/gu, "").toLowerCase();
}

/** Whether `name` contains every word of `query`, in any order. */
export function matches(name: string, query: string): boolean {
  const words = fold(query).split(/\s+/).filter((w) => w !== "");
  const folded = fold(name);
  return words.every((w) => folded.includes(w));
}

/** Shows the filter box of a file table and filters its rows as one types. */
export function setUpFilter(root: ParentNode): void {
  const box = root.querySelector<HTMLElement>("[data-filter]");
  const input = box?.querySelector<HTMLInputElement>("input");
  const table = root.querySelector<HTMLTableElement>("table[data-files]");
  if (!box || !input || !table) return;
  box.hidden = false;
  const empty = root.querySelector<HTMLElement>("[data-filter-empty]");
  const rows = Array.from(table.tBodies[0]?.rows ?? []);
  const apply = (): void => {
    let shown = 0;
    for (const row of rows) {
      const visible = matches(row.dataset["name"] ?? "", input.value);
      row.hidden = !visible;
      if (visible) shown += 1;
    }
    if (empty) empty.hidden = shown > 0 || rows.length === 0;
  };
  input.addEventListener("input", apply);
  input.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && input.value !== "") {
      input.value = "";
      apply();
    }
  });
  apply();
}
