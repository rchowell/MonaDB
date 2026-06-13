#!/usr/bin/env node
/**
 * Generates site/content/examples/ from tests/suites/*.yaml — one page per suite.
 * Run from the site/ directory: node scripts/gen-examples.mjs
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import yaml from "js-yaml";
import {
  renderLiteralsPage,
  renderSuitePage,
  slugFromFilename,
} from "./render-suites.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const siteDir = path.resolve(__dirname, "..");
const repoRoot = path.resolve(siteDir, "..");
const suitesDir = path.join(repoRoot, "tests", "suites");
const outDir = path.join(siteDir, "content", "examples");
const legacyPage = path.join(siteDir, "content", "examples.md");

function writeIndex(firstSlug) {
  const content = `+++
title = "Examples"
description = "Runnable query examples from the MonaDB conformance test suite."
sort_by = "weight"
page_template = "examples/page.html"
redirect_to = "examples/${firstSlug}"
+++
`;
  fs.writeFileSync(path.join(outDir, "_index.md"), content);
}

function main() {
  fs.mkdirSync(outDir, { recursive: true });

  const files = fs
    .readdirSync(suitesDir)
    .filter((f) => f.endsWith(".yaml"))
    .sort();

  const writtenSlugs = new Set();

  for (let i = 0; i < files.length; i++) {
    const file = files[i];
    const slug = slugFromFilename(file);
    writtenSlugs.add(slug);

    const raw = fs.readFileSync(path.join(suitesDir, file), "utf8");
    const suite = yaml.load(raw);
    const { title, body } =
      slug === "literals"
        ? renderLiteralsPage(suite)
        : renderSuitePage(suite, file);

    const desc = suite.description
      ? String(suite.description).trim().replace(/\s+/g, " ")
      : `${title} examples from the conformance test suite.`;

    const frontMatter = `+++
title = "${title.replace(/"/g, '\\"')}"
description = "${desc.replace(/"/g, '\\"')}"
weight = ${i + 1}
+++

# ${title}

`;

    const outPath = path.join(outDir, `${slug}.md`);
    fs.writeFileSync(outPath, frontMatter + body);
  }

  writeIndex(slugFromFilename(files[0]));

  for (const entry of fs.readdirSync(outDir)) {
    if (entry === "_index.md" || !entry.endsWith(".md")) continue;
    const slug = entry.replace(/\.md$/, "");
    if (!writtenSlugs.has(slug)) {
      fs.unlinkSync(path.join(outDir, entry));
    }
  }

  if (fs.existsSync(legacyPage)) {
    fs.unlinkSync(legacyPage);
  }

  console.log(`Wrote ${writtenSlugs.size} pages to ${outDir}`);
}

main();
