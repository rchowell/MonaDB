# Zola Site Migration Design

**Date:** 2026-06-11  
**Status:** Approved

## Overview

Migrate the MonaDB GitHub Pages site from static HTML files to Zola, a single-binary Rust SSG. The landing page stays as hand-authored HTML inside a Zola template; the language reference and design pages become markdown. GitHub Actions builds and deploys on push to main.

## Directory Structure

```
config.toml
content/
  _index.md               # landing page (no body content — template does all rendering)
  design.md               # single-doc architecture overview, markdown
  language/
    _index.md             # section index, redirects to introduction
    introduction.md
    statements.md
    syntax.md
    types.md
    expressions.md
    functions.md
templates/
  base.html               # shared header, footer, nav
  index.html              # landing — extends base, contains existing custom HTML
  page.html               # design page — extends base, renders {{ page.content }}
  language/
    page.html             # language pages — extends base, adds left sidebar nav
static/
  style.css               # existing stylesheet, copied unchanged
.github/workflows/pages.yml
```

## Pages

### Landing (`/`)
- Source: `templates/index.html` extending `base.html`
- Content: existing `index.html` body ported directly — hero, pipeline diagram, code sample, cards
- No markdown involved; the template is the page

### Design (`/design/`)
- Source: `content/design.md`
- Template: `templates/page.html`
- Content: existing `design.html` prose ported to markdown; the cards grid becomes a markdown list or simple prose
- Single page, no sidebar

### Language Reference (`/language/<section>/`)
- Source: `content/language/<section>.md` (one file per section)
- Template: `templates/language/page.html`
- Sidebar: auto-generated from the section's pages in `config.toml` order using Zola's `get_section()` — links highlight the active page
- Sections in order: Introduction, Statements, Syntax, Types, Expressions, Functions
- Initial content: placeholder markdown scaffolded from existing `docs.html`

## Templates

### `base.html`
Contains the `<html>`, `<head>`, `<body>` shell, the `<header>` with nav (brand + Home / Language / Design / GitHub links), the `<footer>`, and a `{% block content %}` slot. All other templates extend this.

### `index.html`
Extends `base.html`. The `{% block content %}` holds the full landing page body exactly as it exists in `index.html` today.

### `page.html`
Extends `base.html`. Renders `{{ page.content | safe }}` inside a `<main><div class="wrap">` shell. Used by `design.md`.

### `language/page.html`
Extends `base.html`. Two-column layout: left sidebar listing all language sections (active item highlighted), right column renders `{{ page.content | safe }}`. The sidebar is built by calling `get_section(path="language/_index.md")` and iterating its pages.

## Styling

`static/style.css` is copied verbatim. Any new styles needed for the sidebar are added to the same file. No Sass required.

## URL Structure

| Page | URL |
|---|---|
| Landing | `/` |
| Design | `/design/` |
| Language: Introduction | `/language/introduction/` |
| Language: Statements | `/language/statements/` |
| Language: Syntax | `/language/syntax/` |
| Language: Types | `/language/types/` |
| Language: Expressions | `/language/expressions/` |
| Language: Functions | `/language/functions/` |

## GitHub Actions Workflow

Replace the current `pages.yml` (which uploads `site/` directly) with a build + deploy workflow:

1. `actions/checkout@v4`
2. Install Zola via `taiki-e/install-action` (or `wget` from GitHub releases)
3. Run `zola build` — outputs to `public/`
4. `actions/configure-pages@v5`
5. `actions/upload-pages-artifact@v3` with `path: public`
6. `actions/deploy-pages@v4`

Trigger: push to `main` on paths `content/**`, `templates/**`, `static/**`, `config.toml`, `.github/workflows/pages.yml`.

## Migration Steps (high-level)

1. Install Zola locally and verify `zola serve` works
2. Create `config.toml`
3. Port `style.css` → `static/style.css`
4. Build `base.html` from the shared header/footer
5. Port `index.html` → `templates/index.html` + `content/_index.md`
6. Build `templates/page.html`, port `design.html` → `content/design.md`
7. Build `templates/language/page.html` with sidebar
8. Scaffold language markdown files from existing `docs.html`
9. Update `.github/workflows/pages.yml`
10. Remove old `site/` directory
11. Test `zola serve` locally, push, verify deploy

## Out of Scope

- Sass / CSS preprocessing
- Search
- RSS / sitemap (Zola generates sitemap.xml by default; that's fine)
- Any JavaScript
