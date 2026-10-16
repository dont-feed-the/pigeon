// Builds the web client into OUT (ADR 0077): type-checks the TypeScript,
// bundles src/main.ts and src/style.css with content-hashed names, copies
// the icon likewise, bundles test/*.test.ts for `node --test`, and writes
// OUT/manifest.json naming the three assets. Deterministic: the same
// sources and lockfile give the same bytes. Usage: node build.mjs OUT
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";

const here = dirname(fileURLToPath(import.meta.url));
const out = resolve(process.argv[2] ?? "dist");
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

const tsc = join(here, "node_modules", "typescript", "bin", "tsc");
const checked = spawnSync(process.execPath, [tsc, "-p", here, "--noEmit"], { stdio: "inherit" });
if (checked.status !== 0) {
  console.error("build.mjs: the TypeScript of the web client does not type-check");
  process.exit(1);
}

const common = { bundle: true, logLevel: "warning", legalComments: "none", charset: "utf8" };
const app = await esbuild.build({
  ...common,
  absWorkingDir: here,
  entryPoints: { app: "src/main.ts", style: "src/style.css" },
  outdir: out,
  entryNames: "[name]-[hash]",
  format: "esm",
  target: ["es2022", "firefox115", "safari16"],
  minify: true,
  metafile: true,
});
const names = Object.keys(app.metafile.outputs).map((p) => p.split("/").pop());
const script = names.find((n) => n.startsWith("app-") && n.endsWith(".js"));
const style = names.find((n) => n.startsWith("style-") && n.endsWith(".css"));

const iconBytes = readFileSync(join(here, "src", "icon.svg"));
const icon = `icon-${createHash("sha256").update(iconBytes).digest("hex").slice(0, 8)}.svg`;
copyFileSync(join(here, "src", "icon.svg"), join(out, icon));

const tests = readdirSync(join(here, "test")).filter((f) => f.endsWith(".test.ts")).sort();
await esbuild.build({
  ...common,
  absWorkingDir: here,
  entryPoints: tests.map((f) => join("test", f)),
  outdir: join(out, "test"),
  outExtension: { ".js": ".mjs" },
  platform: "node",
  format: "esm",
  target: "node22",
});

writeFileSync(join(out, "manifest.json"), JSON.stringify({ script, style, icon }, null, 2) + "\n");
