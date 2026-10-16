// The web client of pigeon's local interface (ADR 0076): progressive
// enhancement of the pages pigeond renders. Every page works without it;
// with it, the status is live, the Files browser opens details and runs
// actions without page loads, names can be filtered, and times are local.
import { setUpBrowse, setUpHistory } from "./browse";
import { setUpCopy } from "./copy";
import { setUpFilter } from "./filter";
import { setUpLive } from "./live";
import { localizeTimes } from "./times";
import { setUpTree } from "./tree";

function enhance(root: ParentNode): void {
  setUpFilter(root);
  setUpCopy(root);
  setUpTree(root);
  localizeTimes(root, document.documentElement.lang || "en");
  setUpBrowse(root, enhance);
}

function start(): void {
  document.documentElement.classList.add("js");
  setUpLive(document);
  setUpHistory(enhance);
  enhance(document);
}

if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
else start();
