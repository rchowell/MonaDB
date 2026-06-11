# Zola Site Migration Design

**Date:** 2026-06-11  
**Status:** Approved (updated — GRAIN design system + all-new copy)

## Overview

Migrate the MonaDB GitHub Pages site from static HTML files to Zola. All copy is freshly written (not ported from placeholders). The visual design implements the GRAIN design system from `docs/resources/design-system.md`: a print-inspired language built on halftone pattern, ink/paper palette, and three editorial typefaces. GitHub Actions builds and deploys on push to main.

---

## Directory Structure

```
config.toml
content/
  _index.md                 # landing (no body — template renders all)
  design.md                 # architecture overview, markdown, all-new copy
  language/
    _index.md               # redirect_to = "language/introduction"
    introduction.md
    statements.md
    syntax.md
    types.md
    expressions.md
    functions.md
templates/
  base.html                 # shell: <html>, <head>, nav, footer, GRAIN CSS vars
  index.html                # landing — extends base, custom hero + sections
  page.html                 # design page — extends base, renders markdown content
  language/
    page.html               # language pages — extends base, sidebar + content
static/
  style.css                 # full GRAIN implementation (replaces existing file)
  grain.js                  # canvas halftone renderer (hero + card images)
.github/workflows/pages.yml
```

---

## GRAIN Design System Implementation

### Color Tokens (CSS custom properties on `:root`)

```
--pa: #EBE7D8   Paper    — page background, default surface
--ik: #1C1A14   Ink      — primary text, borders, filled elements
--gh: #C8C3B5   Ghost    — dividers, light rules, disabled
--mi: #8E8979   Mid      — labels, metadata, eyebrows, captions
--sh: #454039   Shadow   — secondary body text, muted content
--pr: #4A5245   Sage — single accent; focus states, accent buttons
```

### Border Tokens

```
--r:  1.5px solid #1C1A14   Regular — default section rule
--rh: 3px   solid #1C1A14   Heavy   — top caps, blockquote bars
--rl: 1px   solid #C8C3B5   Light   — cell rules, sub-dividers
```

### Typography

Three typefaces, loaded from Google Fonts. Roles are strict — no mixing outside their assignments.

| Role    | Family           | CSS Var | Use                                           |
|---------|------------------|---------|-----------------------------------------------|
| Display | Playfair Display | `--fd`  | Headings, logotype, blockquote body, titles   |
| Body    | Courier Prime    | `--fb`  | All reading-length prose, code prose          |
| Label   | Space Mono       | `--fl`  | Nav, eyebrows, metadata, button labels (ALLCAPS, tracked) |

Base: `--fb` at 14px, line-height 1.65. Eyebrow/nav: `text-transform: uppercase`, `letter-spacing: 0.12–0.14em`.

### Patterns

Static patterns are pure CSS via `radial-gradient` / `repeating-linear-gradient`. Classes: `.ht10`, `.ht25`, `.ht50`, `.ht75`, `.htdiag`, `.hthline`, `.htcross`, `.htaccent`.

### Canvas Halftone

`grain.js` exposes:
- `halftoneGradient(ctx, W, H, invert)` — hero banner ramp; square-root radius scaling
- `halftoneCardImg(ctx, W, H, type)` — card images; wave-modulated density

The hero `<canvas>` on the landing page uses `halftoneGradient`. No smooth CSS gradients anywhere.

### Constraints (hard violations)

- No `box-shadow` with blur radius, no `filter: blur()`, no `text-shadow`
- No smooth CSS gradients in UI elements — patterns or canvas only
- No Inter, Roboto, SF Pro, or similar grotesques
- Sage (`--pr`) used at most once per composition
- All text nodes in elements that explicitly set `font-family`

---

## Pages

### Landing (`/`)

**Template:** `templates/index.html` extending `base.html`  
**Copy:** All-new. Structure:

1. **Hero** — canvas halftone ramp behind the logotype; tagline in Playfair Display; two GRAIN buttons (Primary + Secondary) linking to Language and Design
2. **Pipeline** — `SQL text → Lexer → Parser → IR → Compiler → Vop → VM → LMDB` rendered as a ruled, Space Mono label strip
3. **A taste of RQL** — short code block with GRAIN `<pre>` styling; brief copy introducing the dialect
4. **Principles** — 4-card grid using `.gn-card` with halftone canvas image areas; each card covers one design pillar (Documents, Familiar verbs, Bytecode VM, Embedded)

No markdown. The template is the page.

### Design (`/design/`)

**Source:** `content/design.md`  
**Template:** `templates/page.html`  
**Copy:** All-new prose covering the full pipeline from source text to storage. Sections: Lexer, Parser, IR, Compiler, VM, Storage. Written in Courier Prime body voice — technical, dry, precise. Blocks and inline code in GRAIN `<pre>` / `<code>` style.

### Language Reference (`/language/<section>/`)

**Source:** `content/language/*.md` (one file per section)  
**Template:** `templates/language/page.html`  
**Sidebar:** Auto-built from `get_section(path="language/_index.md")` — ordered list of page titles, active item distinguished by Sage border-left.  
**Copy:** All-new. DuckDB-inspired reference structure:

| Section      | Content                                                         |
|-------------|------------------------------------------------------------------|
| Introduction | What RQL is, how it relates to SQL, the clause model overview   |
| Statements   | SELECT, INSERT, UPDATE, CREATE TABLE, COPY — syntax + examples  |
| Syntax       | Identifiers, reserved words, literals, comments, semicolons     |
| Types        | boolean, number, string, array, object — aliases + nullability  |
| Expressions  | Operators, precedence, objects, paths, cast/coerce              |
| Functions    | Function call syntax, built-in functions, read()                |

---

## Templates

### `base.html`

`<html>` shell with:
- Google Fonts `<link>` for Playfair Display (400/700/900/italic), Courier Prime (400/700), Space Mono (400)
- `<link rel="stylesheet" href="/style.css">`
- `<script src="/grain.js" defer>`
- Sticky nav: brand in Playfair Display Black `letter-spacing: 0.18em`; nav links in Space Mono uppercase `0.12em` tracking
- `{% block content %}` slot
- Footer: Space Mono label pair — left "MonaDB" / right GitHub link

### `index.html`

Extends `base.html`. The `{% block content %}` holds the full landing body with inline data attributes needed by `grain.js` to find its canvas targets.

### `page.html`

Extends `base.html`. Wraps `{{ page.content | safe }}` in a max-width prose column. Headings rendered via GRAIN type scale. Code blocks get GRAIN `<pre>` styling (Ghost border, Paper bg, Ink text, Courier Prime).

### `language/page.html`

Extends `base.html`. Two-column layout:
- Left: sidebar (fixed-width, Ghost right-border) — section list built from `get_section()`; active page gets Sage left-border
- Right: `{{ page.content | safe }}` prose column

---

## Styling (`static/style.css`)

Replaces the existing file entirely. Contains:

1. CSS custom properties (`:root` block — all GRAIN tokens)
2. Base reset and document defaults
3. Navigation and footer
4. GRAIN pattern classes (`.ht10` through `.htcross`)
5. Component classes: `.gn-btn`, `.gn-tag`, `.gn-card`, `.gn-bq`, `.gn-input`
6. Page-specific layout: landing hero, pipeline strip, card grid
7. Language page: sidebar + prose column two-column layout
8. Typography scale applied to markdown-rendered content (h1–h4, p, code, pre, table, blockquote)

---

## URL Structure

| Page                | URL                         |
|--------------------|-----------------------------|
| Landing            | `/`                         |
| Design             | `/design/`                  |
| Language: Intro    | `/language/introduction/`   |
| Language: Stmts    | `/language/statements/`     |
| Language: Syntax   | `/language/syntax/`         |
| Language: Types    | `/language/types/`          |
| Language: Exprs    | `/language/expressions/`    |
| Language: Functions| `/language/functions/`      |

---

## GitHub Actions Workflow

Replace current `pages.yml`:

1. `actions/checkout@v4`
2. Install Zola binary (via `wget` from GitHub releases — pinned version)
3. `zola build` → outputs to `public/`
4. `actions/configure-pages@v5`
5. `actions/upload-pages-artifact@v3` with `path: public`
6. `actions/deploy-pages@v4`

Trigger: push to `main` on paths `content/**`, `templates/**`, `static/**`, `config.toml`, `.github/workflows/pages.yml`.

---

## Implementation Order

1. Install Zola locally, verify binary
2. Create `config.toml`
3. Write `static/style.css` (GRAIN full implementation)
4. Write `static/grain.js` (halftone canvas renderer)
5. Build `templates/base.html`
6. Build `templates/index.html` — landing, all-new copy, canvas hero
7. Build `templates/page.html`
8. Write `content/design.md` — all-new copy
9. Build `templates/language/page.html` — sidebar
10. Scaffold all 6 `content/language/*.md` — all-new copy
11. Update `.github/workflows/pages.yml`
12. Remove old `site/` directory
13. Test `zola serve`, verify GRAIN rendering, push

---

## Out of Scope

- Sass / CSS preprocessing
- Search
- JavaScript beyond `grain.js`
- Sitemap customization (Zola's default is fine)
