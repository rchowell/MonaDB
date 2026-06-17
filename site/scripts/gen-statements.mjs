#!/usr/bin/env node
/**
 * Generates site/content/language/statements/ from conformance suites.
 * Run from the site/ directory: node scripts/gen-statements.mjs
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { renderCategorizedSuiteFile } from "./render-suites.mjs";
import {
  STATEMENT_PAGES,
  renderRulesSection,
  renderSeeAlso,
  renderSyntaxSection,
} from "./statement-pages.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const siteDir = path.resolve(__dirname, "..");
const suitesDir = path.resolve(siteDir, "..", "tests", "suites");
const outDir = path.join(siteDir, "content", "language", "statements");
const legacyFile = path.join(siteDir, "content", "language", "statements.md");

function escapeToml(value) {
  return value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function renderPage(page) {
  const lines = [page.description, ""];

  lines.push(renderSyntaxSection(page));
  lines.push(renderRulesSection(page));
  lines.push(renderCategorizedSuiteFile(suitesDir, page.file));
  lines.push("");
  lines.push("## See also", "");
  lines.push(renderSeeAlso(page));

  return lines.join("\n").trimEnd() + "\n";
}

function writeIndex(firstSlug) {
  const content = `+++
title = "Statements"
description = "SELECT clauses, INSERT, DELETE, CREATE TABLE, DROP TABLE, and CLEAR."
sort_by = "weight"
page_template = "docs/page.html"
redirect_to = "language/statements/${firstSlug}"
+++
`;
  fs.writeFileSync(path.join(outDir, "_index.md"), content);
}

function shortDescription(text) {
  const sentence = text.match(/^[^.!?]+[.!?]/);
  if (sentence) return sentence[0].replace(/"/g, '\\"');
  return text.slice(0, 140).replace(/"/g, '\\"');
}

function main() {
  fs.mkdirSync(outDir, { recursive: true });

  const writtenSlugs = new Set();

  for (const page of STATEMENT_PAGES) {
    writtenSlugs.add(page.slug);

    const frontMatter = `+++
title = "${escapeToml(page.title)}"
description = "${shortDescription(page.description)}"
weight = ${page.weight}
+++

`;

    const outPath = path.join(outDir, `${page.slug}.md`);
    fs.writeFileSync(outPath, frontMatter + renderPage(page));
  }

  writeIndex(STATEMENT_PAGES[0].slug);

  for (const entry of fs.readdirSync(outDir)) {
    if (entry === "_index.md" || !entry.endsWith(".md")) continue;
    const slug = entry.replace(/\.md$/, "");
    if (!writtenSlugs.has(slug)) {
      fs.unlinkSync(path.join(outDir, entry));
    }
  }

  if (fs.existsSync(legacyFile)) {
    fs.unlinkSync(legacyFile);
  }

  console.log(`Wrote ${writtenSlugs.size} pages to ${outDir}`);
}

main();
