// The Files browser without page loads: a row's details open in the side
// panel, and actions marked `data-inline` post in the background, say
// their result in a toast and refresh the page's content. Every link and
// form works the same without this script, by loading a page.

/** A parsed result page: its title, its lines, and whether it is a problem. */
export interface Outcome {
  title: string;
  lines: string[];
  problem: boolean;
}

/** The outcome an action's result page says. */
export function outcome(doc: Document): Outcome {
  const main = doc.getElementById("main");
  const title = (doc.getElementById("title")?.textContent ?? "").trim();
  const lines = Array.from(main?.querySelectorAll("[data-result] li") ?? []).map((li) =>
    (li.textContent ?? "").trim(),
  );
  return { title, lines, problem: main?.querySelector("[data-problem]") !== null };
}

/** Whether a click should be left to the browser (new tab, download…). */
export function leaveToBrowser(e: MouseEvent): boolean {
  return e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey;
}

type Enhance = (root: ParentNode) => void;

async function fetchPage(url: string, init?: RequestInit): Promise<Document | null> {
  const response = await fetch(url, { credentials: "same-origin", ...init });
  const type = response.headers.get("content-type") ?? "";
  if (!type.startsWith("text/html")) return null;
  return new DOMParser().parseFromString(await response.text(), "text/html");
}

/** Replaces this page's content with that of `url`, keeping the sidebar. */
async function refresh(url: string, enhance: Enhance): Promise<void> {
  const next = await fetchPage(url);
  const main = document.getElementById("main");
  const fresh = next?.getElementById("main");
  if (!next || !main || !fresh) {
    location.assign(url);
    return;
  }
  main.replaceChildren(...Array.from(fresh.childNodes).map((n) => document.importNode(n, true)));
  main.className = fresh.className;
  document.title = next.title;
  enhance(main);
}

function toast(o: Outcome): void {
  const box = document.getElementById("toasts");
  if (!box) return;
  const item = document.createElement("div");
  item.className = o.problem ? "toast toast-problem" : "toast";
  const strong = document.createElement("strong");
  strong.textContent = o.title;
  item.append(strong);
  for (const line of o.lines) {
    const p = document.createElement("p");
    p.textContent = line;
    item.append(p);
  }
  box.append(item);
  window.setTimeout(() => item.remove(), o.problem ? 15000 : 8000);
}

/** Opens or closes the details panel from `href` without a page load. */
async function showDetails(href: string, enhance: Enhance): Promise<void> {
  await refresh(href, enhance);
  history.pushState({ details: true }, "", href);
  const heading = document.getElementById("details-title");
  if (heading) heading.focus();
}

/** Wires the browser's links, rows and inline forms under `root`. */
export function setUpBrowse(root: ParentNode, enhance: Enhance): void {
  for (const link of Array.from(root.querySelectorAll<HTMLAnchorElement>("a[data-details]"))) {
    link.addEventListener("click", (e) => {
      if (leaveToBrowser(e)) return;
      e.preventDefault();
      void showDetails(link.href, enhance);
    });
  }
  for (const row of Array.from(root.querySelectorAll<HTMLTableRowElement>("tr[data-details-href]"))) {
    row.addEventListener("click", (e) => {
      const target = e.target as Element | null;
      if (leaveToBrowser(e) || target?.closest("a, button, input, select, label, form")) return;
      void showDetails(row.dataset["detailsHref"] ?? location.href, enhance);
    });
  }
  for (const form of Array.from(root.querySelectorAll<HTMLFormElement>("form[data-inline]"))) {
    form.addEventListener("submit", (e) => {
      e.preventDefault();
      const button = form.querySelector<HTMLButtonElement>("button[type=submit]");
      if (button) button.disabled = true;
      const body = new URLSearchParams();
      for (const [k, v] of new FormData(form)) if (typeof v === "string") body.append(k, v);
      void fetchPage(form.action, { method: "POST", body })
        .then(async (page) => {
          if (page) toast(outcome(page));
          await refresh(location.href, enhance);
        })
        .catch(() => form.submit());
    });
  }
}

/** Follows the browser's back and forward buttons between panel states. */
export function setUpHistory(enhance: Enhance): void {
  window.addEventListener("popstate", () => void refresh(location.href, enhance));
}
