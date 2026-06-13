#!/usr/bin/env node
/**
 * Generates site/content/language/statements.md from conformance suites.
 * Run from the site/ directory: node scripts/gen-statements.mjs
 */

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import { renderSuiteFile } from "./render-suites.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const siteDir = path.resolve(__dirname, "..");
const repoRoot = path.resolve(siteDir, "..");
const suitesDir = path.join(repoRoot, "tests", "suites");
const outFile = path.join(siteDir, "content", "language", "statements.md");
const outDir = path.join(siteDir, "content", "language", "statements");

const SELECT_SYNTAX = `\`\`\`
select <constructor>
  [from <source> [, <source> …]]
  [where <expr>]
  [order by <expr> [asc|desc] [, …]]
  [limit <n> | limit <n>.. | limit <n>..<m>];
\`\`\``;

const SELECT_CLAUSES = [
  {
    title: "Select",
    file: "08-select.yaml",
    syntax: SELECT_SYNTAX,
    description:
      "The Select clause maps the current binding stream through a constructor: an expression, an object literal, a list of named expressions, `*` to spread bound variables, or `.` to wrap each binding under its alias.",
  },
  {
    title: "From",
    file: "09-from.yaml",
    description:
      "The From clause iterates sources — table scans, array literals, and lateral unnest paths — binding each row under an alias for use in later clauses.",
  },
  {
    title: "Unpivot",
    file: "17-unpivot.yaml",
    syntax: `\`\`\`
from unpivot <expr> [as <value>] [at <name>]
\`\`\``,
    description:
      "The Unpivot clause is a from-source that ranges over the attribute-value pairs of a tuple, binding each pair's value with `as` and its attribute name with `at` — the dual of Pivot.",
  },
  {
    title: "Pivot",
    file: "18-pivot.yaml",
    syntax: `\`\`\`
pivot <value> at <name> from <source> [where <expr>];
\`\`\``,
    description:
      "The Pivot clause replaces select, folding the whole binding stream into a single tuple: each row contributes one `name: value` member — the dual of Unpivot.",
  },
  {
    title: "Where",
    file: "10-where.yaml",
    description:
      "The Where clause filters binding tuples by a boolean predicate. Only rows for which the predicate is true pass through.",
  },
  {
    title: "Order by",
    file: "15-order.yaml",
    description:
      "The Order by clause sorts the binding-tuple stream by one or more keys. Ascending is the default; nulls sort last in ascending order and first in descending order.",
  },
  {
    title: "Limit",
    file: "12-limit.yaml",
    description:
      "The Limit clause slices the stream by row position: `limit n` takes the first n rows, `limit n..` skips the first n, and `limit n..m` selects the half-open index range [n, m).",
  },
];

const STATEMENT_SECTIONS = [
  {
    title: "Insert",
    syntax: `\`\`\`
insert into <table> (<value>, …);
\`\`\``,
    file: "06-insert.yaml",
    description:
      "Insert adds one or more values to a table. The values list is parenthesised and comma-separated; a trailing comma is permitted.",
  },
  {
    title: "Delete",
    syntax: `\`\`\`
delete from <table> [as <alias>] [where <expr>];
\`\`\``,
    file: "07-delete.yaml",
    description:
      "Delete removes rows from a table. Without a Where clause, every row is removed.",
  },
  {
    title: "Create Table",
    syntax: `\`\`\`
create table <name> [(<key> int|string, …)];
\`\`\``,
    file: "13-keys.yaml",
    description:
      "Create Table declares a table with an optional list of key columns. Key columns must be `int` or `string` and define the physical sort order; keyless tables keep surrogate ids and return rows in insertion order.",
  },
  {
    title: "Drop Table",
    syntax: `\`\`\`
drop table <name>;
\`\`\``,
    file: "07-drop.yaml",
    description:
      "Drop Table removes a table and all its contents from the catalog.",
  },
  {
    title: "Clear",
    syntax: `\`\`\`
clear table <name>;
\`\`\``,
    file: "07-clear.yaml",
    description:
      "Clear removes every row from a table but keeps the table definition.",
  },
];

function removeStatementsDir() {
  if (!fs.existsSync(outDir)) return;
  for (const entry of fs.readdirSync(outDir)) {
    fs.unlinkSync(path.join(outDir, entry));
  }
  fs.rmdirSync(outDir);
}

function renderSection(title, parts) {
  return [`## ${title}`, "", ...parts].join("\n");
}

function sectionParts({ description, syntax, file }) {
  const parts = [description, ""];
  if (syntax) {
    parts.push(syntax, "");
  }
  parts.push(renderSuiteFile(suitesDir, file));
  return parts;
}

function main() {
  const sections = [];

  for (const clause of SELECT_CLAUSES) {
    sections.push(renderSection(clause.title, sectionParts(clause)));
  }

  for (const stmt of STATEMENT_SECTIONS) {
    sections.push(renderSection(stmt.title, sectionParts(stmt)));
  }

  const frontMatter = `+++
title = "Statements"
description = "SELECT, INSERT, DELETE, CREATE TABLE, DROP TABLE, and CLEAR."
weight = 2
+++

# Statements

`;

  fs.writeFileSync(outFile, frontMatter + sections.join("\n\n"));
  removeStatementsDir();

  console.log(`Wrote ${outFile}`);
}

main();
