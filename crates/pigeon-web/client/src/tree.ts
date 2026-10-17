// The folder tree of *Folders on this computer*: ticking a folder ticks
// every folder in it, and a folder whose folders are not all as it is
// shows as partly kept. Without this script each box stands alone and the
// server marks the partly kept folders in words.

/** A folder of the tree: whether it is ticked, and its folders. */
export interface Folder {
  checked: boolean;
  children: Folder[];
}

/** Ticks or unticks `folder` and every folder in it. */
export function setAll(folder: Folder, checked: boolean): void {
  folder.checked = checked;
  for (const child of folder.children) setAll(child, checked);
}

/** Whether some folder in `folder` is not ticked as `folder` is. */
export function mixed(folder: Folder): boolean {
  const differs = (f: Folder): boolean => f.checked !== folder.checked || f.children.some(differs);
  return folder.children.some(differs);
}

interface Linked extends Folder {
  box: HTMLInputElement;
  children: Linked[];
}

function read(list: Element | null): Linked[] {
  if (!list) return [];
  const out: Linked[] = [];
  for (const item of Array.from(list.children)) {
    const box = item.querySelector<HTMLInputElement>(":scope > .tree-row input[type=checkbox]");
    if (!box) continue;
    out.push({ box, checked: box.checked, children: read(item.querySelector(":scope > details > ul, :scope > ul")) });
  }
  return out;
}

function show(folders: Linked[]): void {
  for (const f of folders) {
    f.box.checked = f.checked;
    f.box.indeterminate = mixed(f);
    show(f.children);
  }
}

/** Links the boxes of every folder tree under `root`. */
export function setUpTree(root: ParentNode): void {
  for (const tree of Array.from(root.querySelectorAll<HTMLUListElement>("ul[data-tree]"))) {
    const folders = read(tree);
    const all = (fs: Linked[]): Linked[] => fs.flatMap((f) => [f, ...all(f.children)]);
    for (const f of all(folders)) {
      f.box.addEventListener("change", () => {
        setAll(f, f.box.checked);
        show(folders);
      });
    }
    for (const note of Array.from(tree.querySelectorAll<HTMLElement>("[data-mixed-note]"))) note.hidden = true;
    show(folders);
  }
}
