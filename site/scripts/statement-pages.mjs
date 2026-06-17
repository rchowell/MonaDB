/**
 * Metadata for generated statement reference pages.
 */

import { renderRailroad } from "./railroad.mjs";

/** @typedef {{ text: string, phase?: string }} Rule */

/**
 * @typedef {object} StatementPage
 * @property {string} slug
 * @property {string} title
 * @property {string} description
 * @property {number} weight
 * @property {string} file
 * @property {import('./railroad.mjs').RailroadNode[]} railroad
 * @property {string} bnf
 * @property {Rule[]} rules
 * @property {{ title: string, href: string, note?: string }[]} seeAlso
 */

/** @param {Rule[]} rules */
function renderRules(rules) {
  return rules
    .map((rule, index) => {
      const phase = rule.phase
        ? ` *(phase: ${rule.phase})*`
        : "";
      return `${index + 1}. ${rule.text}${phase}`;
    })
    .join("\n");
}

/** @param {StatementPage} page */
export function renderSeeAlso(page) {
  if (!page.seeAlso.length) return "";
  return page.seeAlso
    .map((link) => {
      const note = link.note ? ` — ${link.note}` : "";
      return `- [${link.title}](${link.href})${note}`;
    })
    .join("\n");
}

/** @param {StatementPage} page */
export function renderSyntaxSection(page) {
  const lines = ["## Syntax", "", "### Railroad", "", renderRailroad(page.railroad), ""];
  lines.push("### BNF", "", "```ebnf", page.bnf.trim(), "```", "");
  return lines.join("\n");
}

/** @param {StatementPage} page */
export function renderRulesSection(page) {
  return ["## Rules", "", renderRules(page.rules), ""].join("\n");
}

export const STATEMENT_PAGES = [
  {
    slug: "select",
    title: "Select",
    weight: 1,
    file: "08-select.yaml",
    description:
      "The Select clause is the final projection of a query. It runs once per binding tuple in the current stream and emits one output value per tuple — a scalar, a constructed object, a flat spread of bindings, or an envelope object keyed by source aliases.",
    railroad: [
      { t: "select" },
      {
        or: [
          [{ t: "." }],
          [{ t: "*" }],
          [{ n: "expr" }],
          [{ n: "select-list" }],
        ],
      },
      {
        opt: [
          { t: "from" },
          { n: "source" },
          {
            rep: [{ t: "," }, { n: "source" }],
          },
        ],
      },
      {
        opt: [{ t: "where" }, { n: "expr" }],
      },
      {
        opt: [
          { t: "order" },
          { t: "by" },
          { n: "order-key" },
          {
            rep: [{ t: "," }, { n: "order-key" }],
          },
        ],
      },
      {
        opt: [{ t: "limit" }, { n: "range" }],
      },
      { t: ";" },
    ],
    bnf: `
select-stmt ::= "select" select-ctor query-body-opt ";"

select-ctor ::= "."
              | "*"
              | expr
              | select-list

select-list ::= select-item ( "," select-item )*

select-item ::= expr [ "as" identifier ]

query-body-opt ::= ε | query-body

query-body ::= from-clause [ where-clause ] [ order-clause ] [ limit-clause ]
`.trim(),
    rules: [
      {
        text: "`select expr` emits a scalar per row, not an object.",
        phase: "evaluate last — after **from**, **where**, **order by**, and **limit**",
      },
      {
        text: "`select item, item, …` is shorthand for `select {item, item, …}`; an item `expr as name` introduces an output key, and a path item such as `t.x` uses the last segment as the key.",
      },
      {
        text: "`select *` spreads all bindings flat; `select .` wraps each binding under its source alias.",
      },
      {
        text: "With no `from` clause, the query produces exactly one output row.",
      },
      {
        text: "Aggregate functions are valid only in the select projection (ungrouped aggregation reduces the post-where stream to one row).",
        phase: "evaluate last",
      },
    ],
    seeAlso: [
      { title: "From", href: "@/language/statements/from.md", note: "introduces bindings consumed by select" },
      { title: "Where", href: "@/language/statements/where.md", note: "filters rows before projection" },
      { title: "Expressions", href: "@/language/expressions.md", note: "constructor and projection expressions" },
    ],
  },
  {
    slug: "from",
    title: "From",
    weight: 2,
    file: "09-from.yaml",
    description:
      "The From clause introduces bindings by iterating data sources. Each source binds rows under an alias for use in later clauses. Multiple comma-separated sources form a lateral cross product: each subsequent source is evaluated in the context of bindings from preceding sources.",
    railroad: [
      { t: "from" },
      { n: "source" },
      {
        rep: [{ t: "," }, { n: "source" }],
      },
    ],
    bnf: `
from-clause ::= "from" source ( "," source )*

source ::= table-ref
         | path-expr
         | "(" select-stmt ")"
         | unpivot-source

table-ref ::= identifier [ "as" identifier ]

unpivot-source ::= "unpivot" expr "as" identifier [ "at" identifier ]
`.trim(),
    rules: [
      {
        text: "Each source must have an alias; when `as` is omitted, the alias defaults to the table name.",
        phase: "evaluate first in the query pipeline",
      },
      {
        text: "Sources are evaluated left to right; the right-hand source may reference bindings from the left (lateral evaluation).",
        phase: "evaluate first",
      },
      {
        text: "A source may be a table name, a path expression rooted at a bound name, a parenthesized subquery, or `unpivot`.",
      },
      {
        text: "Scanning an empty table yields no rows; insertion order is preserved for keyless tables.",
        phase: "evaluate first",
      },
    ],
    seeAlso: [
      { title: "Select", href: "@/language/statements/select.md" },
      { title: "Unpivot", href: "@/language/statements/unpivot.md", note: "from-source over object members" },
      { title: "Where", href: "@/language/statements/where.md", note: "filters the binding stream after from" },
    ],
  },
  {
    slug: "unpivot",
    title: "Unpivot",
    weight: 3,
    file: "17-unpivot.yaml",
    description:
      "Unpivot is a from-source that ranges over the attribute–value pairs of an object. Each pair binds its value under the required `as` alias and its attribute name under the optional `at` alias. It is the dual of Pivot.",
    railroad: [
      { t: "unpivot" },
      { n: "expr" },
      { t: "as" },
      { n: "value" },
      {
        opt: [{ t: "at" }, { n: "name" }],
      },
    ],
    bnf: `
unpivot-source ::= "unpivot" expr "as" identifier [ "at" identifier ]
`.trim(),
    rules: [
      {
        text: "The value alias (`as`) is required; the attribute-name alias (`at`) is optional.",
        phase: "evaluate as part of **from**",
      },
      {
        text: "Each object member produces one output row with the value and name bindings.",
        phase: "evaluate as part of **from**",
      },
      {
        text: "When `expr` is not an object, unpivot yields no rows (inner-join semantics).",
        phase: "evaluate as part of **from**",
      },
      {
        text: "Unpivot may appear inline in a comma-separated from list and may reference lateral bindings from preceding sources.",
        phase: "evaluate as part of **from**",
      },
    ],
    seeAlso: [
      { title: "Pivot", href: "@/language/statements/pivot.md", note: "inverse fold into one object" },
      { title: "From", href: "@/language/statements/from.md" },
    ],
  },
  {
    slug: "pivot",
    title: "Pivot",
    weight: 4,
    file: "18-pivot.yaml",
    description:
      "Pivot replaces the select constructor and folds the entire binding stream into a single object. Each surviving tuple contributes one member `name: value`. It is the dual of Unpivot.",
    railroad: [
      { t: "pivot" },
      { n: "value" },
      { t: "at" },
      { n: "name" },
      { t: "from" },
      { n: "source" },
      {
        rep: [{ t: "," }, { n: "source" }],
      },
      {
        opt: [{ t: "where" }, { n: "expr" }],
      },
      { t: ";" },
    ],
    bnf: `
pivot-stmt ::= "pivot" expr "at" expr "from" source ( "," source )* [ where-clause ] ";"
`.trim(),
    rules: [
      {
        text: "Pivot requires a `from` clause and yields exactly one object (one output row).",
        phase: "evaluate last — replaces **select**",
      },
      {
        text: "`name` must evaluate to `string`; tuples whose name is not a string contribute no member.",
        phase: "evaluate last",
      },
      {
        text: "Repeated names are last-wins across the folded stream.",
        phase: "evaluate last",
      },
      {
        text: "An empty input stream yields `{}`.",
        phase: "evaluate last",
      },
      {
        text: "v1 supports `from` and `where` only; `order by` and `limit` on pivot queries are deferred.",
        phase: "evaluate last",
      },
    ],
    seeAlso: [
      { title: "Unpivot", href: "@/language/statements/unpivot.md" },
      { title: "From", href: "@/language/statements/from.md" },
      { title: "Where", href: "@/language/statements/where.md" },
    ],
  },
  {
    slug: "where",
    title: "Where",
    weight: 5,
    file: "10-where.yaml",
    description:
      "The Where clause filters the binding-tuple stream by a boolean predicate. Only tuples for which the predicate evaluates to exactly `true` pass through; `false` and `null` both drop the row.",
    railroad: [
      { t: "where" },
      { n: "expr" },
    ],
    bnf: `
where-clause ::= "where" expr
`.trim(),
    rules: [
      {
        text: "The predicate must evaluate to `bool`. A `null` result is treated as not-true and drops the tuple.",
        phase: "after **from** / **with**, before **group**, **order by**, **limit**, and **select**",
      },
      {
        text: "Comparison with `null` follows SQL three-valued logic: only `null = null` is `true`; `null` compared to a non-null value is `false`; ordering `null` against non-null yields `null` (drops the row in where).",
        phase: "after **from**",
      },
      {
        text: "Aggregate functions may not appear in the where predicate.",
        phase: "after **from**",
      },
    ],
    seeAlso: [
      { title: "From", href: "@/language/statements/from.md" },
      { title: "Expressions", href: "@/language/expressions.md", note: "predicates and operators" },
      { title: "Select", href: "@/language/statements/select.md" },
    ],
  },
  {
    slug: "order-by",
    title: "Order by",
    weight: 6,
    file: "15-order.yaml",
    description:
      "The Order by clause sorts the binding-tuple stream by one or more keys. Ascending is the default. Nulls sort last in ascending order and first in descending order.",
    railroad: [
      { t: "order" },
      { t: "by" },
      { n: "order-key" },
      {
        rep: [{ t: "," }, { n: "order-key" }],
      },
    ],
    bnf: `
order-clause ::= "order" "by" order-key ( "," order-key )*

order-key ::= expr [ ( "asc" | "desc" ) ]
`.trim(),
    rules: [
      {
        text: "Default sort direction is `asc`.",
        phase: "after **where** / **group**, before **limit** and **select**",
      },
      {
        text: "`null` sorts last in `asc` and first in `desc`.",
        phase: "before **limit**",
      },
      {
        text: "Numbers order by numeric value; `1` and `1.0` compare equal.",
        phase: "before **limit**",
      },
      {
        text: "Sort stability is not guaranteed.",
        phase: "before **limit**",
      },
    ],
    seeAlso: [
      { title: "Limit", href: "@/language/statements/limit.md", note: "slice after sorting" },
      { title: "Select", href: "@/language/statements/select.md" },
    ],
  },
  {
    slug: "limit",
    title: "Limit",
    weight: 7,
    file: "12-limit.yaml",
    description:
      "The Limit clause slices the stream by row position using Python-style half-open range syntax. `limit n` takes the first *n* rows; `limit n..` skips the first *n*; `limit n..m` selects the half-open index range [*n*, *m*).",
    railroad: [
      { t: "limit" },
      {
        or: [
          [{ n: "integer" }],
          [{ n: "integer" }, { t: ".." }],
          [{ n: "integer" }, { t: ".." }, { n: "integer" }],
          [
            { n: "integer" },
            { t: ".." },
            { n: "integer" },
            { t: ".." },
            { n: "integer" },
          ],
        ],
      },
    ],
    bnf: `
limit-clause ::= "limit" limit-range

limit-range ::= integer
              | integer ".." [ integer ] [ ".." integer ]
`.trim(),
    rules: [
      {
        text: "`limit n` is shorthand for `limit 0..n`.",
        phase: "after **order by**, before **select**",
      },
      {
        text: "Range `start..end..step` is half-open: indices `start, start+step, …` strictly less than `end` are emitted.",
        phase: "before **select**",
      },
      {
        text: "Omitted `start` defaults to `0`; omitted `end` is unbounded; omitted `step` is `1`.",
        phase: "before **select**",
      },
      {
        text: "`start`, `end`, and `step` must be non-negative integer literals; `step` must be ≥ 1.",
        phase: "parse / static",
      },
    ],
    seeAlso: [
      { title: "Order by", href: "@/language/statements/order-by.md" },
      { title: "Select", href: "@/language/statements/select.md" },
    ],
  },
  {
    slug: "insert",
    title: "Insert",
    weight: 8,
    file: "06-insert.yaml",
    description:
      "Insert adds one or more values to a table. The values list is parenthesised and comma-separated; a trailing comma is permitted. Values may also come from a nested `select` query.",
    railroad: [
      { t: "insert" },
      { t: "into" },
      { n: "table" },
      { t: "(" },
      {
        or: [[{ n: "expr-list" }], [{ n: "select-stmt" }]],
      },
      { t: ")" },
      { t: ";" },
    ],
    bnf: `
insert-stmt ::= "insert" "into" identifier "(" expr-list ")" ";"
              | "insert" "into" identifier select-stmt

expr-list ::= expr ( "," expr )*
`.trim(),
    rules: [
      {
        text: "Each inserted value must be an object that satisfies the table schema; schema mismatch is a runtime error.",
        phase: "execute",
      },
      {
        text: "Duplicate full key replaces the existing row (LMDB put semantics; no `NOOVERWRITE`).",
        phase: "execute",
      },
      {
        text: "Scalar or non-object values in the values list are rejected.",
        phase: "execute",
      },
      {
        text: "A trailing comma after the last value in the list is permitted.",
        phase: "parse",
      },
    ],
    seeAlso: [
      { title: "Create Table", href: "@/language/statements/create-table.md" },
      { title: "Writing data", href: "@/examples/writing.md", note: "more insert examples" },
    ],
  },
  {
    slug: "delete",
    title: "Delete",
    weight: 9,
    file: "07-delete.yaml",
    description:
      "Delete removes rows from a table. An optional `where` predicate restricts which rows are removed; without it, every row in the table is deleted.",
    railroad: [
      { t: "delete" },
      { t: "from" },
      { n: "table" },
      {
        opt: [{ t: "as" }, { n: "alias" }],
      },
      {
        opt: [{ t: "where" }, { n: "expr" }],
      },
      { t: ";" },
    ],
    bnf: `
delete-stmt ::= "delete" "from" identifier [ "as" identifier ] [ where-clause ] ";"
`.trim(),
    rules: [
      {
        text: "Without `where`, every row in the table is removed.",
        phase: "execute",
      },
      {
        text: "The table alias is optional; when omitted, the table name is the binding name in the predicate.",
        phase: "execute",
      },
      {
        text: "The where predicate follows the same boolean semantics as query `where` (null is not-true).",
        phase: "execute",
      },
    ],
    seeAlso: [
      { title: "Where", href: "@/language/statements/where.md" },
      { title: "Clear", href: "@/language/statements/clear.md", note: "remove all rows without a predicate" },
      { title: "Writing data", href: "@/examples/writing.md" },
    ],
  },
  {
    slug: "create-table",
    title: "Create Table",
    weight: 10,
    file: "13-keys.yaml",
    description:
      "Create Table declares a table in the catalog. An optional schema lists key columns (in declaration order) that form the composite physical key. Key columns must be `int` or `string`. Keyless tables accept any object and preserve insertion order via surrogate ids.",
    railroad: [
      { t: "create" },
      { t: "table" },
      { n: "name" },
      {
        opt: [
          { t: "(" },
          {
            or: [
              [{ t: ")" }],
              [{ n: "key-column" }, { rep: [{ t: "," }, { n: "key-column" }] }, { t: ")" }],
            ],
          },
        ],
      },
      { t: ";" },
    ],
    bnf: `
create-table-stmt ::= "create" "table" identifier [ "(" [ key-column ( "," key-column )* ] ")" ] ";"

key-column ::= identifier ( "int" | "string" )
`.trim(),
    rules: [
      {
        text: "Without a schema, the table accepts any JSON object.",
        phase: "catalog",
      },
      {
        text: "Declared columns form the composite key; key columns must be `int` or `string`.",
        phase: "catalog",
      },
      {
        text: "Fields are non-null by default; declare `T | null` to permit null (general unions are not supported).",
        phase: "catalog",
      },
      {
        text: "Creating a table that already exists is a static error (`IF NOT EXISTS` is not supported).",
        phase: "catalog",
      },
      {
        text: "Inserts that violate the declared schema (missing keys, wrong types, extra keys on closed schemas) error at runtime.",
        phase: "execute on insert",
      },
    ],
    seeAlso: [
      { title: "Insert", href: "@/language/statements/insert.md" },
      { title: "Primary Keys", href: "@/examples/keys.md", note: "keyed lookup examples" },
      { title: "Schemas", href: "@/language/schemas.md" },
    ],
  },
  {
    slug: "drop-table",
    title: "Drop Table",
    weight: 11,
    file: "07-drop.yaml",
    description:
      "Drop Table removes a table and all of its rows from the catalog. The table name must exist; dropping a missing table is a static error.",
    railroad: [
      { t: "drop" },
      { t: "table" },
      { n: "name" },
      { t: ";" },
    ],
    bnf: `
drop-table-stmt ::= "drop" "table" identifier ";"
`.trim(),
    rules: [
      {
        text: "Dropping a non-existent table is a static error.",
        phase: "catalog",
      },
      {
        text: "All rows and the table definition are removed.",
        phase: "catalog",
      },
    ],
    seeAlso: [
      { title: "Create Table", href: "@/language/statements/create-table.md" },
      { title: "Clear", href: "@/language/statements/clear.md", note: "empty a table but keep its definition" },
    ],
  },
  {
    slug: "clear",
    title: "Clear",
    weight: 12,
    file: "07-clear.yaml",
    description:
      "Clear removes every row from a table but keeps the table definition in the catalog. It is equivalent to `delete from` without a where clause, but expressed as a dedicated statement.",
    railroad: [
      { t: "clear" },
      { t: "table" },
      { n: "name" },
      { t: ";" },
    ],
    bnf: `
clear-stmt ::= "clear" "table" identifier ";"
`.trim(),
    rules: [
      {
        text: "Clears all rows; the table schema and catalog entry remain.",
        phase: "execute",
      },
      {
        text: "Clearing a non-existent table is a static error.",
        phase: "catalog",
      },
    ],
    seeAlso: [
      { title: "Delete", href: "@/language/statements/delete.md" },
      { title: "Drop Table", href: "@/language/statements/drop-table.md" },
    ],
  },
];
