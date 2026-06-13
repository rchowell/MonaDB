/**
 * Shared rendering for conformance suite YAML → markdown example blocks.
 */

import fs from "fs";
import path from "path";
import yaml from "js-yaml";

export const SUITE_TITLES = {
  literals: "Literals",
  insert: "Insert",
  "select-clause": "Select",
  "from-clause": "From",
  "where-clause": "Where",
  predicates: "Predicates",
  "limit-clause": "Limit",
  keys: "Keys",
  get: "Keyed lookup",
  "order-clause": "Order by",
  aggregate: "Aggregate",
  "unpivot-clause": "Unpivot",
  "pivot-clause": "Pivot",
  functions: "Functions",
  cast: "Casts",
  delete: "Delete",
  drop: "Drop table",
  clear: "Clear table",
};

export function slugFromFilename(filename) {
  return filename.replace(/^\d+-/, "").replace(/\.yaml$/, "");
}

function capitalizeWord(word) {
  if (!word) return word;
  if (word === "null") return "null";
  return word.charAt(0).toUpperCase() + word.slice(1);
}

const SQL_START =
  /^(select|insert|delete|create|drop|clear|update|a)\b/i;

function titleFromDescription(description) {
  const cleaned = description.replace(/[`'"]/g, "").trim();
  if (SQL_START.test(cleaned)) {
    const verb = cleaned.match(
      /\b(emits|returns|yields|produces|removes|requires|filters|sorts|skips|takes|rejects|accepts|permits|spreads|wraps|unnest|iterates|scans|binds|matches|excludes|includes|overwrites|round-trips|evaluates|compiles|executes|declares|clears|empties)\b/i,
    );
    if (verb) {
      const tail = cleaned.slice(verb.index + verb[0].length).trim();
      const stop = new Set([
        "the", "a", "an", "as", "in", "to", "of", "by", "on", "at", "with", "from",
        "and", "or", "not", "no", "one", "every", "each", "all", "any", "only",
      ]);
      const words = tail
        .replace(/[()[\].,;:!?<>]/g, " ")
        .split(/\s+/)
        .filter((w) => w && !stop.has(w.toLowerCase()));
      if (words.length >= 2) {
        return words
          .slice(-2)
          .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
          .join(" ");
      }
    }
    return null;
  }
  const words = cleaned.split(/\s+/).filter(Boolean);
  if (words.length === 0) return null;
  const count =
    words.length === 1 ? 1 : words.length <= 3 ? words.length : 2;
  return words
    .slice(0, count)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(" ");
}

function titleFromId(id) {
  const parts = id.split("-");
  if (parts.length <= 3) {
    return parts.map(capitalizeWord).join(" ");
  }
  if (parts[0] === "is" || parts[0] === "not" || parts[0] === "fn") {
    return parts.slice(0, 3).map(capitalizeWord).join(" ");
  }
  return parts.slice(-2).map(capitalizeWord).join(" ");
}

function exampleTitle(test) {
  if (test.description) {
    const fromDesc = titleFromDescription(test.description);
    if (fromDesc) return fromDesc;
  }
  return titleFromId(test.id);
}

function capitalizeClauseRoles(text) {
  const replacements = [
    [/\border by clause\b/gi, "Order by clause"],
    [/\bcreate table statement\b/gi, "Create Table statement"],
    [/\bdrop table statement\b/gi, "Drop Table statement"],
    [/\bselect clause\b/gi, "Select clause"],
    [/\bfrom clause\b/gi, "From clause"],
    [/\bwhere clause\b/gi, "Where clause"],
    [/\blimit clause\b/gi, "Limit clause"],
    [/\binsert statement\b/gi, "Insert statement"],
    [/\bdelete statement\b/gi, "Delete statement"],
    [/\bclear statement\b/gi, "Clear statement"],
    [/\bfrom source\b/gi, "From source"],
  ];
  let result = text;
  for (const [pattern, replacement] of replacements) {
    result = result.replace(pattern, replacement);
  }
  return result;
}

function exampleDescription(test) {
  if (test.description) {
    let d = test.description.trim().replace(/\s+/g, " ");
    d = d.charAt(0).toUpperCase() + d.slice(1);
    d = capitalizeClauseRoles(d);
    if (!/[.!?]$/.test(d)) d += ".";
    return d;
  }
  const phrase = test.id.replace(/-/g, " ");
  return `Shows the result of \`${phrase}\`.`;
}

function exampleLabel(text) {
  return `<p class="example-label">${text}</p>`;
}

/** Compact JSON: one result row per line, objects/arrays inlined. */
function compactJsonValue(value) {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    if (value.length === 0) return "[]";
    return `[ ${value.map(compactJsonValue).join(", ")} ]`;
  }
  const entries = Object.entries(value).map(
    ([k, v]) => `"${k}": ${compactJsonValue(v)}`,
  );
  if (entries.length === 0) return "{}";
  return `{ ${entries.join(", ")} }`;
}

function resultToJson(result) {
  if (!Array.isArray(result)) {
    return compactJsonValue(result);
  }
  if (result.length === 0) return "[]";
  const lines = result.map((row) => compactJsonValue(row));
  return `[\n  ${lines.join(",\n  ")}\n]`;
}

function sqlBlock(statements, { terse = false } = {}) {
  const body = statements.map((s) => s.trim()).join("\n\n");
  if (terse) return `\`\`\`sql\n${body}\n\`\`\``;
  return `${exampleLabel("SQL")}\n\n\`\`\`sql\n${body}\n\`\`\``;
}

/** Single-row select results document the value, not the row array. */
function unwrapScalarResult(result) {
  if (Array.isArray(result) && result.length === 1) return result[0];
  return result;
}

function resultBlock(result, { terse = false, scalar = false } = {}) {
  const json = scalar ? compactJsonValue(unwrapScalarResult(result)) : resultToJson(result);
  if (terse) return `\`\`\`json\n${json}\n\`\`\``;
  return `${exampleLabel("Result")}\n\n\`\`\`json\n${json}\n\`\`\``;
}

function collectSetup(suite, test) {
  const setup = [];
  for (const stmt of suite.setup ?? []) setup.push(stmt);
  for (const stmt of test.setup ?? []) setup.push(stmt);
  return setup;
}

function collectTestExamples(test, suite) {
  const setup = collectSetup(suite, test);
  const positive = [];
  const errors = [];
  let session = [...setup];

  for (const step of test.steps ?? []) {
    const sql = typeof step.sql === "string" ? step.sql : String(step.sql);
    if (step.result !== undefined) {
      positive.push({ session: [...session, sql], result: step.result });
      session.push(sql);
    } else if (step.error !== undefined) {
      errors.push({ session: [...session, sql], error: step.error });
      session.push(sql);
    } else {
      session.push(sql);
    }
  }

  return { positive, errors, session };
}

export function renderTest(test, suite, { terse = false } = {}) {
  const lines = [];
  const { positive, errors, session } = collectTestExamples(test, suite);

  if (!terse) {
    const title = exampleTitle(test);
    const description = exampleDescription(test);
    lines.push('<div class="example">');
    lines.push("");
    lines.push(`### ${title}`);
    lines.push("");
    lines.push(description);
    lines.push("");
  }

  if (positive.length === 0 && errors.length === 0) {
    if (session.length > 0) {
      lines.push(sqlBlock(session, { terse }));
      lines.push("");
    }
    if (!terse) lines.push("</div>");
    return lines.join("\n").trimEnd();
  }

  for (const ex of positive) {
    lines.push(sqlBlock(ex.session, { terse }));
    lines.push("");
    lines.push(resultBlock(ex.result, { terse }));
    lines.push("");
  }

  for (const ex of errors) {
    lines.push(sqlBlock(ex.session, { terse }));
    lines.push("");
    lines.push(`Expected error: \`${ex.error}\``);
    lines.push("");
  }

  if (!terse) lines.push("</div>");
  return lines.join("\n").trimEnd();
}

export function renderSuiteTests(suite, { terse = false } = {}) {
  const lines = [];
  for (const test of suite.tests ?? []) {
    lines.push(renderTest(test, suite, { terse }));
    lines.push("");
  }
  return lines.join("\n").trimEnd();
}

/** Curated literals page — one section per kind, variations grouped. */
export function renderLiteralsPage(suite) {
  const sections = [
    {
      title: "Null",
      examples: [{ sql: "select null;", result: [null] }],
    },
    {
      title: "Boolean",
      examples: [
        { sql: "select true;", result: [true] },
        { sql: "select false;", result: [false] },
      ],
    },
    {
      title: "Number",
      examples: [
        { sql: "select 1;", result: [1] },
        { sql: "select 1.5;", result: [1.5] },
        { sql: "select 9007199254740992;", result: [9007199254740992] },
      ],
    },
    {
      title: "String",
      examples: [
        { sql: "select 'hello';", result: ["hello"] },
        { sql: "select '';", result: [""] },
        { sql: "select 'café';", result: ["café"] },
      ],
    },
    {
      title: "Array",
      examples: [
        { sql: "select [];", result: [[]] },
        { sql: "select [1, 2, 3];", result: [[1, 2, 3]] },
        { sql: "select [1, 'a', null, true];", result: [[1, "a", null, true]] },
        { sql: "select [[1, 2], [3, 4]];", result: [[[1, 2], [3, 4]]] },
      ],
    },
    {
      title: "Object",
      examples: [
        { sql: "select {};", result: [{}] },
        { sql: "select {x: 1, y: 2};", result: [{ x: 1, y: 2 }] },
        {
          sql: "select {items: [1, 2], meta: {n: 2}};",
          result: [{ items: [1, 2], meta: { n: 2 } }],
        },
      ],
    },
  ];

  const lines = [];
  for (const section of sections) {
    lines.push('<div class="example">');
    lines.push("");
    lines.push(`## ${section.title}`);
    lines.push("");
    for (const ex of section.examples) {
      lines.push(sqlBlock([ex.sql]));
      lines.push("");
      lines.push(resultBlock(ex.result, { scalar: true }));
      lines.push("");
    }
    lines.push("</div>");
    lines.push("");
  }

  const name = suite.suite ?? "literals";
  const title = SUITE_TITLES[name] ?? name;
  return { title, body: lines.join("\n").trimEnd() };
}

export function renderSuitePage(suite, filename) {
  const name = suite.suite ?? path.basename(filename, ".yaml");
  const title = SUITE_TITLES[name] ?? name;
  const lines = [];

  if (suite.description) {
    const desc = String(suite.description).trim().replace(/\s+/g, " ");
    lines.push(desc);
    lines.push("");
  }

  lines.push(renderSuiteTests(suite));

  return { title, body: lines.join("\n").trimEnd() };
}

/** Render a single suite file into markdown example blocks. */
export function renderSuiteFile(suitesDir, file, { terse = false } = {}) {
  const suite = loadSuite(suitesDir, file);
  return renderSuiteTests(suite, { terse });
}

/** Render one or more suite files into a single examples body. */
export function renderSuitesFromFiles(
  suitesDir,
  files,
  { sectionTitles = {}, sectionHeading = "##", terse = false } = {},
) {
  const sections = [];

  for (const file of files) {
    const suite = loadSuite(suitesDir, file);
    const name = suite.suite ?? slugFromFilename(file);
    const sectionTitle = sectionTitles[name] ?? SUITE_TITLES[name] ?? name;

    if (!terse && files.length > 1) {
      sections.push(`${sectionHeading} ${sectionTitle}`);
      sections.push("");
    }

    sections.push(renderSuiteTests(suite, { terse }));
    sections.push("");
  }

  return sections.join("\n").trimEnd();
}

export function loadSuite(suitesDir, file) {
  const raw = fs.readFileSync(path.join(suitesDir, file), "utf8");
  return yaml.load(raw);
}
