// Times in the person's own time zone (ux.md §4): the server writes dates
// older than a day as `<time datetime=… data-date>` with a plain date in
// UTC; this shows them in the page's language and local time, with the
// exact time on hover.

/** The date of `iso` for `lang`, local; within this year without the year. */
export function localDate(iso: string, lang: string, now: Date, timeZone?: string): string | null {
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) return null;
  const zone = timeZone === undefined ? {} : { timeZone };
  const sameYear =
    new Intl.DateTimeFormat("en", { year: "numeric", ...zone }).format(at) ===
    new Intl.DateTimeFormat("en", { year: "numeric", ...zone }).format(now);
  const options: Intl.DateTimeFormatOptions = sameYear
    ? { day: "numeric", month: "short", ...zone }
    : { day: "numeric", month: "short", year: "numeric", ...zone };
  return new Intl.DateTimeFormat(lang, options).format(at);
}

/** The full local date and time of `iso` for `lang`, for a tooltip. */
export function exactTime(iso: string, lang: string, timeZone?: string): string | null {
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) return null;
  const zone = timeZone === undefined ? {} : { timeZone };
  return new Intl.DateTimeFormat(lang, { dateStyle: "full", timeStyle: "short", ...zone }).format(at);
}

/** Rewrites every `time[datetime]` under `root`. */
export function localizeTimes(root: ParentNode, lang: string): void {
  const now = new Date();
  for (const t of Array.from(root.querySelectorAll<HTMLTimeElement>("time[datetime]"))) {
    const iso = t.dateTime;
    const exact = exactTime(iso, lang);
    if (exact) t.title = exact;
    if (t.hasAttribute("data-date")) {
      const date = localDate(iso, lang, now);
      if (date) t.textContent = date;
    }
  }
}
