// Tests of the web client's logic that needs no page: the filter's
// matching, local dates, the levels and health events of the live status,
// and which clicks are left to the browser. Run by `cargo nextest` through
// pigeon-web's `the_client_logic_passes_its_tests`, or `npm test`.
import assert from "node:assert/strict";
import { test } from "node:test";
import { leaveToBrowser } from "../src/browse";
import { fold, matches } from "../src/filter";
import { level, parseHealth } from "../src/live";
import { exactTime, localDate } from "../src/times";

test("the filter ignores case, accents and the order of words", () => {
  assert.equal(fold("Été À Noël"), "ete a noel");
  assert.ok(matches("Vacances d'été 2019.jpg", "ETE 2019"));
  assert.ok(matches("Vacances d'été 2019.jpg", "2019 vacances"));
  assert.ok(matches("anything", "   "));
  assert.ok(!matches("Vacances 2019.jpg", "2020"));
});

test("dates are local, without the year within this year", () => {
  const now = new Date("2026-10-16T12:00:00Z");
  assert.equal(localDate("2026-10-12T08:00:00Z", "en-GB", now, "UTC"), "12 Oct");
  assert.equal(localDate("2025-03-02T08:00:00Z", "en-GB", now, "UTC"), "2 Mar 2025");
  assert.equal(localDate("2026-10-12T23:30:00Z", "fr", now, "Europe/Paris"), "13 oct.");
  assert.equal(localDate("not a time", "en", now), null);
  assert.match(exactTime("2026-10-12T08:00:00Z", "en", "UTC") ?? "", /Monday, 12 October 2026|Monday, October 12, 2026/);
});

test("only known levels become classes", () => {
  assert.equal(level("safe"), "safe");
  assert.equal(level("needs-you"), "needs-you");
  assert.equal(level("safe lv-evil"), null);
});

test("a health event is read only when it has its three parts", () => {
  const ok = parseHealth(JSON.stringify({ membership: "[]", values: { a: "b" }, explanations: ["x", 1] }));
  assert.deepEqual(ok, { membership: "[]", values: { a: "b" }, explanations: ["x", "1"] });
  assert.equal(parseHealth("{"), null);
  assert.equal(parseHealth(JSON.stringify({ values: {}, explanations: [] })), null);
  assert.equal(parseHealth("null"), null);
});

test("clicks with a modifier, or not with the main button, stay the browser's", () => {
  const click = (over: Partial<MouseEvent>): MouseEvent =>
    ({ defaultPrevented: false, button: 0, metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, ...over }) as MouseEvent;
  assert.equal(leaveToBrowser(click({})), false);
  assert.equal(leaveToBrowser(click({ ctrlKey: true })), true);
  assert.equal(leaveToBrowser(click({ metaKey: true })), true);
  assert.equal(leaveToBrowser(click({ button: 1 })), true);
  assert.equal(leaveToBrowser(click({ defaultPrevented: true })), true);
});
