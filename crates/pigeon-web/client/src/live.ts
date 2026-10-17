// Live status (10 §10.3): the server's events refresh the headline in the
// sidebar on every page and, on Home, the headline and the health facts.
// Only text is replaced; a change of membership reloads Home.

/** The headline levels (06 §6.5), as the server names them. */
export const LEVELS = ["needs-you", "not-safe", "not-checked", "working", "safe"] as const;
export type Level = (typeof LEVELS)[number];

/** `name` if it is a level, else null. */
export function level(name: string): Level | null {
  return (LEVELS as readonly string[]).includes(name) ? (name as Level) : null;
}

/** What a `health` event holds. */
export interface Health {
  membership: string;
  values: Record<string, string>;
  explanations: string[];
}

/** The `health` event's data, or null if it is not one. */
export function parseHealth(data: string): Health | null {
  try {
    const p: unknown = JSON.parse(data);
    if (typeof p !== "object" || p === null) return null;
    const h = p as Partial<Health>;
    if (typeof h.membership !== "string" || typeof h.values !== "object" || h.values === null) return null;
    if (!Array.isArray(h.explanations)) return null;
    return { membership: h.membership, values: h.values, explanations: h.explanations.map(String) };
  } catch {
    return null;
  }
}

function setText(node: Element | null, text: string): void {
  if (node && node.textContent !== text) node.textContent = text;
}

/**
 * Listens to the page's event stream, if it names one, while the page is
 * shown: a browser allows few connections to one server, and a stream
 * holds one, so hidden tabs and left pages give theirs back.
 */
export function setUpLive(doc: Document): void {
  const url = doc.body.dataset["events"];
  if (!url || typeof EventSource === "undefined") return;
  let source: EventSource | null = null;
  const open = (): void => {
    if (source || doc.visibilityState === "hidden") return;
    source = new EventSource(url);
    listen(doc, source);
  };
  const close = (): void => {
    source?.close();
    source = null;
  };
  doc.addEventListener("visibilitychange", () => (doc.visibilityState === "hidden" ? close() : open()));
  window.addEventListener("pagehide", close);
  window.addEventListener("pageshow", open);
  open();
}

function listen(doc: Document, source: EventSource): void {
  const pill = doc.getElementById("live-status");
  source.addEventListener("headline", (e) => {
    const text = (e as MessageEvent<string>).data;
    setText(doc.getElementById("headline"), text);
    setText(doc.getElementById("live-headline"), text);
    if (pill) pill.hidden = false;
  });
  source.addEventListener("level", (e) => {
    const l = level((e as MessageEvent<string>).data);
    for (const node of [pill, doc.getElementById("headline-card")]) {
      if (!node || !l) continue;
      for (const other of LEVELS) node.classList.toggle(`lv-${other}`, other === l);
    }
  });
  source.addEventListener("health", (e) => {
    const p = parseHealth((e as MessageEvent<string>).data);
    const section = doc.getElementById("health");
    if (!p || !section) return;
    if (section.dataset["membership"] !== p.membership) {
      location.reload();
      return;
    }
    for (const [id, value] of Object.entries(p.values)) setText(doc.getElementById(id), value);
    const list = doc.getElementById("health-explanations");
    if (!list) return;
    const now = Array.from(list.children).map((n) => n.textContent ?? "");
    if (JSON.stringify(now) === JSON.stringify(p.explanations)) return;
    list.replaceChildren(
      ...p.explanations.map((text) => {
        const li = doc.createElement("li");
        li.textContent = text;
        return li;
      }),
    );
  });
}
