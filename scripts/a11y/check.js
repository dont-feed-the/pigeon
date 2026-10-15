// axe-core over HTML files (10 §10.3, ux.md §3): each file is loaded in
// jsdom with its own stylesheets, and every WCAG 2.2 A/AA rule that runs
// without a layout engine is checked. Colour contrast needs layout, so it
// is checked by the mockups' review, not here. Exits 1 on any violation.
"use strict";
const fs = require("fs");
const path = require("path");
const { JSDOM, VirtualConsole } = require("jsdom");
const axeSource = fs.readFileSync(require.resolve("axe-core/axe.min.js"), "utf8");

async function check(file) {
  const html = fs.readFileSync(file, "utf8");
  const quiet = new VirtualConsole();
  const dom = new JSDOM(html, {
    url: "file://" + path.resolve(file),
    resources: "usable",
    runScripts: "outside-only",
    pretendToBeVisual: true,
    virtualConsole: quiet,
  });
  await new Promise((done) => {
    if (dom.window.document.readyState === "complete") done();
    else dom.window.addEventListener("load", done);
  });
  dom.window.eval(axeSource);
  const result = await dom.window.axe.run(dom.window.document, {
    runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"] },
    rules: { "color-contrast": { enabled: false } },
  });
  dom.window.close();
  return result.violations;
}

(async () => {
  const files = process.argv.slice(2).flatMap((p) =>
    fs.statSync(p).isDirectory()
      ? fs.readdirSync(p).filter((f) => f.endsWith(".html")).sort().map((f) => path.join(p, f))
      : [p],
  );
  if (files.length === 0) {
    console.error("check-a11y: no HTML file");
    process.exit(2);
  }
  let bad = 0;
  for (const f of files) {
    const violations = await check(f);
    for (const v of violations) {
      bad += 1;
      console.log(`${f}: ${v.id} (${v.impact}): ${v.help}`);
      for (const n of v.nodes.slice(0, 3)) console.log(`    ${n.target.join(" ")}`);
    }
  }
  console.log(`check-a11y: ${files.length} pages, ${bad} violations`);
  process.exit(bad === 0 ? 0 : 1);
})();
