#!/usr/bin/env node
/**
 * Generates site/content/examples/ from tests/suites/*.yaml — five consolidated pages.
 * Run from the site/ directory: node scripts/gen-examples.mjs
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import {
  loadSuite,
  renderLiteralsPage,
  renderSuitesFromFiles,
  slugFromFilename,
} from "./render-suites.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const siteDir = path.resolve(__dirname, "..");
const repoRoot = path.resolve(siteDir, "..");
const suitesDir = path.join(repoRoot, "tests", "suites");
const outDir = path.join(siteDir, "content", "examples");
const legacyPage = path.join(siteDir, "content", "examples.md");

/** Consolidated example pages — each merges several conformance suites. */
const CONSOLIDATED_PAGES = [
  {
    slug: "queries",
    title: "Queries",
    description:
      "Select, from, where, order, limit, group, aggregate, and subquery examples.",
    weight: 1,
    intro:
      "Reading and transforming data — select shapes output, from iterates sources, and subsequent clauses filter, sort, group, or nest results.",
    files: [
      "08-select.yaml",
      "09-from.yaml",
      "10-where.yaml",
      "15-order.yaml",
      "12-limit.yaml",
      "16-aggregate.yaml",
      "19-group.yaml",
      "21-subquery.yaml",
    ],
  },
  {
    slug: "writing",
    title: "Writing data",
    description: "Insert, delete, clear, and drop examples.",
    weight: 2,
    intro: "Creating tables, inserting rows, and removing data.",
    files: [
      "06-insert.yaml",
      "07-delete.yaml",
      "07-clear.yaml",
      "07-drop.yaml",
    ],
  },
  {
    slug: "keys",
    title: "Keys & lookups",
    description: "Key columns, ordering, and keyed get examples.",
    weight: 3,
    intro: "Declaring key columns and fetching rows by key.",
    files: ["13-keys.yaml", "14-get.yaml"],
  },
  {
    slug: "values",
    title: "Values & functions",
    description: "Literals, predicates, casts, and built-in functions.",
    weight: 4,
    intro: "Expressions evaluated in isolation or inside larger queries.",
    files: [
      "01-literals.yaml",
      "11-predicates.yaml",
      "17-cast.yaml",
      "16-functions.yaml",
    ],
    literals: true,
  },
  {
    slug: "reshape",
    title: "Pivot & reshape",
    description: "Pivot, unpivot, and parameter binding examples.",
    weight: 5,
    intro: "Reshaping row streams and binding host parameters.",
    files: ["18-pivot.yaml", "17-unpivot.yaml", "20-params.yaml"],
  },
];

function writeIndex() {
  const content = `+++
title = "Examples"
description = "Runnable query examples from the MonaDB conformance test suite."
sort_by = "weight"
page_template = "docs/page.html"
redirect_to = "examples/queries"
+++
`;
  fs.writeFileSync(path.join(outDir, "_index.md"), content);
}

function renderPage(page) {
  const lines = [`# ${page.title}`, "", page.intro, ""];

  if (page.literals) {
    const literalsSuite = loadSuite(suitesDir, "01-literals.yaml");
    const { body: literalsBody } = renderLiteralsPage(literalsSuite);
    lines.push(literalsBody, "");
    const rest = page.files.filter((f) => f !== "01-literals.yaml");
    if (rest.length > 0) {
      lines.push(
        renderSuitesFromFiles(suitesDir, rest, { sectionHeading: "##" }),
      );
    }
  } else {
    lines.push(renderSuitesFromFiles(suitesDir, page.files, { sectionHeading: "##" }));
  }

  return lines.join("\n").trimEnd() + "\n";
}

function main() {
  fs.mkdirSync(outDir, { recursive: true });

  const writtenSlugs = new Set();

  for (const page of CONSOLIDATED_PAGES) {
    writtenSlugs.add(page.slug);

    const frontMatter = `+++
title = "${page.title.replace(/"/g, '\\"')}"
description = "${page.description.replace(/"/g, '\\"')}"
weight = ${page.weight}
+++

`;

    const outPath = path.join(outDir, `${page.slug}.md`);
    fs.writeFileSync(outPath, frontMatter + renderPage(page));
  }

  writeIndex();

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
