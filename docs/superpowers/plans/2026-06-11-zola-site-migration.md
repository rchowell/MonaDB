# Zola Site Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the MonaDB GitHub Pages site from static HTML to Zola, applying the GRAIN design system throughout with all-new copy and a multi-section language reference with sidebar navigation.

**Architecture:** Zola SSG with Tera templates. Landing page is custom HTML inside `templates/index.html`; design and language pages are markdown in `content/`. GRAIN design system implemented in `static/style.css` and `static/grain.js`. GitHub Actions builds on push to main and uploads `public/` to Pages.

**Tech Stack:** Zola v0.19.2+, Tera templates, CSS (no Sass), vanilla JS canvas halftone, GitHub Actions

---

### Task 1: Scaffold Zola project skeleton

**Files:**
- Create: `config.toml`
- Create: `content/_index.md`
- Create: `content/design.md` (placeholder)
- Create: `content/language/_index.md`
- Create: `content/language/introduction.md` (placeholder)
- Create: `content/language/statements.md` (placeholder)
- Create: `content/language/syntax.md` (placeholder)
- Create: `content/language/types.md` (placeholder)
- Create: `content/language/expressions.md` (placeholder)
- Create: `content/language/functions.md` (placeholder)
- Create: `templates/base.html` (stub)
- Create: `templates/index.html` (stub)
- Create: `templates/page.html` (stub)
- Create: `templates/language/page.html` (stub)
- Create: `.gitignore` entry for `public/`

- [ ] **Step 1: Verify Zola is installed**

```bash
zola --version
```

If missing on macOS: `brew install zola`. Expected: `zola 0.19.x` or later.

- [ ] **Step 2: Create config.toml**

```toml
base_url = "https://rchowell.github.io/MonaDB"
title = "MonaDB"
compile_sass = false
generate_feeds = false
minify_html = false

[markdown]
highlight_code = true
highlight_theme = "base16-ocean-dark"

[extra]
github = "https://github.com/rchowell/MonaDB"
```

- [ ] **Step 3: Create content/_index.md**

```toml
+++
title = "MonaDB"
description = "An embedded database with a query language of its own."
+++
```

- [ ] **Step 4: Create content/design.md**

```toml
+++
title = "Design"
description = "How MonaDB compiles query text to bytecode and executes it against LMDB."
+++

placeholder
```

- [ ] **Step 5: Create content/language/_index.md**

```toml
+++
title = "Language"
description = "RQL language reference."
sort_by = "weight"
redirect_to = "language/introduction"
+++
```

- [ ] **Step 6: Create all six language placeholder pages**

`content/language/introduction.md`:
```toml
+++
title = "Introduction"
description = "What RQL is, how it relates to SQL, and how to read this reference."
weight = 1
+++

placeholder
```

`content/language/statements.md`:
```toml
+++
title = "Statements"
description = "SELECT, INSERT, UPDATE, CREATE TABLE, and COPY."
weight = 2
+++

placeholder
```

`content/language/syntax.md`:
```toml
+++
title = "Syntax"
description = "Identifiers, reserved words, literals, comments, and semicolons."
weight = 3
+++

placeholder
```

`content/language/types.md`:
```toml
+++
title = "Types"
description = "Scalar types, collection types, aliases, and nullability."
weight = 4
+++

placeholder
```

`content/language/expressions.md`:
```toml
+++
title = "Expressions"
description = "Operators, precedence, object constructors, path traversal, and type coercion."
weight = 5
+++

placeholder
```

`content/language/functions.md`:
```toml
+++
title = "Functions"
description = "Function call syntax, built-in aggregate functions, and read()."
weight = 6
+++

placeholder
```

- [ ] **Step 7: Create stub templates so Zola can build**

`templates/base.html`:
```html
<!DOCTYPE html><html><body>{% block content %}{% endblock content %}</body></html>
```

`templates/index.html`:
```html
{% extends "base.html" %}
{% block content %}landing{% endblock content %}
```

`templates/page.html`:
```html
{% extends "base.html" %}
{% block content %}{{ page.content | safe }}{% endblock content %}
```

`templates/language/page.html`:
```html
{% extends "base.html" %}
{% block content %}{{ page.content | safe }}{% endblock content %}
```

- [ ] **Step 8: Add public/ to .gitignore**

Append to `.gitignore` (create if it doesn't exist):
```
public/
```

- [ ] **Step 9: Run zola check**

```bash
zola check
```

Expected: completes without errors (may warn about external links — that is fine).

- [ ] **Step 10: Commit skeleton**

```bash
git add config.toml content/ templates/ .gitignore
git commit -m "feat: zola project skeleton"
```

---

### Task 2: Write grain.js — canvas halftone renderer

**Files:**
- Create: `static/grain.js`

- [ ] **Step 1: Write static/grain.js**

```js
(function () {
  function halftoneGradient(canvas, invert) {
    var ctx = canvas.getContext('2d');
    var W = canvas.width, H = canvas.height;
    ctx.fillStyle = '#EBE7D8';
    ctx.fillRect(0, 0, W, H);
    ctx.fillStyle = '#1C1A14';
    var g = 7;
    for (var y = g / 2; y < H; y += g) {
      for (var x = g / 2; x < W; x += g) {
        var raw = x / W;
        var d = invert ? 1 - raw : raw;
        var r = (g / 2 - 0.5) * Math.sqrt(d);
        if (r > 0.2) {
          ctx.beginPath();
          ctx.arc(x, y, r, 0, Math.PI * 2);
          ctx.fill();
        }
      }
    }
  }

  function halftoneCard(canvas, type) {
    var ctx = canvas.getContext('2d');
    var W = canvas.width, H = canvas.height;
    ctx.fillStyle = '#EBE7D8';
    ctx.fillRect(0, 0, W, H);
    ctx.fillStyle = '#1C1A14';
    var g = type === 'dot50' ? 6 : 7;
    var pct = type === 'dot50' ? 0.45 : 0.27;
    for (var y = g / 2; y < H; y += g) {
      for (var x = g / 2; x < W; x += g) {
        var wave = Math.sin((x / W) * Math.PI) * 0.7 + Math.sin((y / H) * Math.PI * 2) * 0.15;
        var d = pct * (0.4 + wave * 0.6);
        var r = (g / 2 - 0.3) * Math.sqrt(Math.max(0, d / pct));
        if (r > 0.3) {
          ctx.beginPath();
          ctx.arc(x, y, r, 0, Math.PI * 2);
          ctx.fill();
        }
      }
    }
  }

  function init() {
    document.querySelectorAll('canvas[data-grain]').forEach(function (canvas) {
      var type = canvas.dataset.grain;
      var parent = canvas.parentElement;
      canvas.width = parent ? parent.offsetWidth : 800;
      var h = parseInt(canvas.dataset.height || '0', 10);
      canvas.height = h || canvas.offsetHeight || 120;
      if (type === 'hero') halftoneGradient(canvas, false);
      else if (type === 'hero-inv') halftoneGradient(canvas, true);
      else halftoneCard(canvas, type);
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
}());
```

- [ ] **Step 2: Commit**

```bash
git add static/grain.js
git commit -m "feat: grain.js canvas halftone renderer"
```

---

### Task 3: Write style.css — GRAIN design system

**Files:**
- Replace: `static/style.css`

- [ ] **Step 1: Replace static/style.css with full GRAIN implementation**

```css
@import url('https://fonts.googleapis.com/css2?family=Playfair+Display:ital,wght@0,400;0,700;0,900;1,400;1,700&family=Courier+Prime:ital,wght@0,400;0,700;1,400&family=Space+Mono:wght@400&display=swap');

/* ── GRAIN tokens ──────────────────────────────────────── */
:root {
  --pa: #EBE7D8;
  --ik: #1C1A14;
  --gh: #C8C3B5;
  --mi: #8E8979;
  --sh: #454039;
  --pr: #5C6B77;
  --fd: 'Playfair Display', Georgia, serif;
  --fb: 'Courier Prime', 'Courier New', monospace;
  --fl: 'Space Mono', 'Courier New', monospace;
  --r:  1.5px solid #1C1A14;
  --rh: 3px   solid #1C1A14;
  --rl: 1px   solid #C8C3B5;
}

/* ── Reset ─────────────────────────────────────────────── */
*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: var(--pa);
  color: var(--ik);
  font-family: var(--fb);
  font-size: 14px;
  line-height: 1.65;
}
a { color: inherit; text-decoration: none; }
a:hover { text-decoration: underline; }
canvas, img { display: block; max-width: 100%; }

/* ── Nav ───────────────────────────────────────────────── */
.site-nav {
  position: sticky; top: 0; z-index: 10;
  background: var(--pa);
  border-bottom: var(--r);
}
.nav-inner {
  max-width: 960px; margin: 0 auto; padding: 0 2rem;
  height: 48px; display: flex; align-items: center; justify-content: space-between;
}
.nav-brand {
  font-family: var(--fd); font-weight: 900; font-size: 1.05rem;
  letter-spacing: 0.18em; color: var(--ik);
}
.nav-dot { color: var(--pr); }
.nav-links { display: flex; align-items: center; gap: 1.8rem; }
.nav-links a {
  font-family: var(--fl); font-size: 0.6rem;
  letter-spacing: 0.12em; text-transform: uppercase; color: var(--mi);
}
.nav-links a:hover, .nav-links a.nav-active { color: var(--ik); text-decoration: none; }
.nav-links .nav-github { color: var(--ik); }

/* ── Footer ────────────────────────────────────────────── */
.site-foot { border-top: var(--r); margin-top: 4rem; }
.foot-inner {
  max-width: 960px; margin: 0 auto; padding: 1.2rem 2rem;
  display: flex; justify-content: space-between; align-items: center;
}
.foot-name, .foot-link {
  font-family: var(--fl); font-size: 0.58rem;
  letter-spacing: 0.1em; text-transform: uppercase; color: var(--mi);
}
.foot-link:hover { color: var(--ik); text-decoration: none; }

/* ── GRAIN Patterns ────────────────────────────────────── */
.ht10 { background: var(--pa); background-image: radial-gradient(circle, var(--ik) 13%, transparent 13%); background-size: 9px 9px; }
.ht25 { background: var(--pa); background-image: radial-gradient(circle, var(--ik) 27%, transparent 27%); background-size: 7px 7px; }
.ht50 { background: var(--pa); background-image: radial-gradient(circle, var(--ik) 45%, transparent 45%); background-size: 6px 6px; }
.ht75 { background: var(--pa); background-image: radial-gradient(circle, var(--ik) 65%, transparent 65%); background-size: 5px 5px; }
.htdiag  { background: var(--pa); background-image: repeating-linear-gradient(45deg, var(--ik) 0, var(--ik) 1px, transparent 1px, transparent 5px); }
.hthline { background: var(--pa); background-image: repeating-linear-gradient(0deg,  var(--ik) 0, var(--ik) 1px, transparent 1px, transparent 5px); }
.htcross { background: var(--pa); background-image: repeating-linear-gradient(0deg, var(--ik) 0, var(--ik) 1px, transparent 0, transparent 5px), repeating-linear-gradient(90deg, var(--ik) 0, var(--ik) 1px, transparent 0, transparent 5px); background-size: 5px 5px; }

/* ── Buttons ───────────────────────────────────────────── */
.gn-btn {
  font-family: var(--fl); font-size: 0.62rem; letter-spacing: 0.1em;
  text-transform: uppercase; padding: 0.45rem 1.1rem; border: var(--r);
  background: none; color: var(--ik); cursor: pointer; display: inline-block;
}
.gn-btn:hover { text-decoration: none; opacity: 0.75; }
.gn-btn-pri { background: var(--ik); color: var(--pa); }
.gn-btn-sec { background: transparent; color: var(--ik); }

/* ── Landing: Hero ─────────────────────────────────────── */
.hero { position: relative; overflow: hidden; border-bottom: var(--r); }
.hero-canvas { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
.hero-inner { position: relative; max-width: 960px; margin: 0 auto; padding: 5rem 2rem 4.5rem; }
.hero-eyebrow {
  font-family: var(--fl); font-size: 0.58rem; letter-spacing: 0.14em;
  text-transform: uppercase; color: var(--mi); margin-bottom: 1.2rem;
}
.hero-title {
  font-family: var(--fd); font-size: 2.8rem; font-weight: 900;
  line-height: 1.0; max-width: 640px; margin-bottom: 1.4rem;
}
.hero-lead {
  font-family: var(--fb); font-size: 0.95rem; color: var(--sh);
  max-width: 520px; line-height: 1.65; margin-bottom: 2rem;
}
.hero-cta { display: flex; gap: 0.8rem; flex-wrap: wrap; }

/* ── Landing: Section shell ────────────────────────────── */
.landing-sect { border-bottom: var(--r); padding: 2.5rem 0; }
.landing-inner { max-width: 960px; margin: 0 auto; padding: 0 2rem; }
.sect-eyebrow {
  font-family: var(--fl); font-size: 0.58rem; letter-spacing: 0.14em;
  text-transform: uppercase; color: var(--mi); margin-bottom: 0.6rem;
}
.sect-title {
  font-family: var(--fd); font-size: 1.8rem; font-weight: 700;
  line-height: 1.1; margin-bottom: 1.8rem;
}

/* ── Landing: Pipeline ─────────────────────────────────── */
.pipeline {
  display: flex; align-items: stretch; flex-wrap: wrap;
  border: var(--r); overflow: hidden;
}
.pipeline-step {
  font-family: var(--fl); font-size: 0.58rem; letter-spacing: 0.1em;
  text-transform: uppercase; padding: 0.6rem 0.9rem;
  border-right: var(--r); white-space: nowrap; color: var(--ik);
}
.pipeline-step:last-child { border-right: none; }

/* ── Landing: Code block ───────────────────────────────── */
.landing-pre {
  border: var(--r); background: var(--pa);
  padding: 1.2rem 1.4rem; overflow-x: auto;
  font-family: var(--fb); font-size: 0.85rem; line-height: 1.6; color: var(--ik);
}

/* ── Landing: Principle cards ──────────────────────────── */
.principles-grid {
  display: grid; grid-template-columns: 1fr 1fr;
  border: var(--r); overflow: hidden;
}
.gn-card { border-right: var(--r); border-bottom: var(--r); }
.gn-card:nth-child(2n)       { border-right: none; }
.gn-card:nth-last-child(-n+2){ border-bottom: none; }
.gn-card-img { width: 100%; height: 90px; display: block; }
.gn-card-body { padding: 1rem 1.1rem; }
.gn-card-title {
  font-family: var(--fd); font-size: 1rem; font-weight: 700;
  line-height: 1.25; margin-bottom: 0.5rem;
}
.gn-card-body p {
  font-family: var(--fb); font-size: 0.82rem; color: var(--sh); line-height: 1.5;
}

/* ── Prose page (design) ───────────────────────────────── */
.prose-page { max-width: 720px; margin: 0 auto; padding: 3rem 2rem; }
.prose-page h1 {
  font-family: var(--fd); font-size: 2.8rem; font-weight: 900; line-height: 1.0;
  margin-bottom: 2rem; padding-bottom: 1rem; border-bottom: var(--rh);
}
.prose-page h2 {
  font-family: var(--fd); font-size: 1.8rem; font-weight: 700; line-height: 1.1;
  margin-top: 2.5rem; margin-bottom: 0.8rem; padding-bottom: 0.5rem; border-bottom: var(--r);
}
.prose-page h3 {
  font-family: var(--fd); font-size: 1.25rem; font-weight: 700;
  margin-top: 1.8rem; margin-bottom: 0.5rem;
}
.prose-page p {
  font-family: var(--fb); font-size: 0.95rem; color: var(--sh); line-height: 1.65; margin-bottom: 1rem;
}
.prose-page pre {
  border: var(--r); background: var(--pa); padding: 1rem 1.2rem;
  overflow-x: auto; margin-bottom: 1rem;
}
.prose-page pre code {
  font-family: var(--fb); font-size: 0.82rem; color: var(--ik); background: none; padding: 0;
}
.prose-page code {
  font-family: var(--fb); font-size: 0.85rem; padding: 0.05em 0.25em; border: var(--rl);
}
.prose-page table {
  width: 100%; border-collapse: collapse; border: var(--r); margin-bottom: 1rem;
  font-family: var(--fb); font-size: 0.85rem;
}
.prose-page th {
  font-family: var(--fl); font-size: 0.58rem; letter-spacing: 0.1em; text-transform: uppercase;
  color: var(--mi); padding: 0.5rem 0.8rem; border-bottom: var(--r); text-align: left;
}
.prose-page td { padding: 0.45rem 0.8rem; border-bottom: var(--rl); }
.prose-page tr:last-child td { border-bottom: none; }
.prose-page a { color: var(--pr); }
.prose-page a:hover { text-decoration: underline; }

/* ── Language page: sidebar + content ─────────────────── */
.lang-page {
  max-width: 960px; margin: 0 auto;
  display: grid; grid-template-columns: 200px 1fr;
  min-height: calc(100vh - 48px);
}
.lang-sidebar { border-right: var(--r); padding: 2rem 0; }
.sidebar-heading {
  font-family: var(--fl); font-size: 0.56rem; letter-spacing: 0.14em;
  text-transform: uppercase; color: var(--mi); padding: 0 1.2rem; margin-bottom: 0.6rem;
}
.sidebar-nav { list-style: none; }
.sidebar-nav a {
  display: block; font-family: var(--fl); font-size: 0.6rem; letter-spacing: 0.08em;
  text-transform: uppercase; color: var(--mi); padding: 0.45rem 1.2rem;
  border-left: 3px solid transparent;
}
.sidebar-nav a:hover { color: var(--ik); text-decoration: none; }
.sidebar-nav a.active { color: var(--ik); border-left-color: var(--pr); }
.lang-content { padding: 3rem 2.5rem; }

/* lang-content prose (mirrors prose-page) */
.lang-content h1 {
  font-family: var(--fd); font-size: 2.8rem; font-weight: 900; line-height: 1.0;
  margin-bottom: 2rem; padding-bottom: 1rem; border-bottom: var(--rh);
}
.lang-content h2 {
  font-family: var(--fd); font-size: 1.8rem; font-weight: 700; line-height: 1.1;
  margin-top: 2.5rem; margin-bottom: 0.8rem; padding-bottom: 0.5rem; border-bottom: var(--r);
}
.lang-content h3 {
  font-family: var(--fd); font-size: 1.25rem; font-weight: 700;
  margin-top: 1.8rem; margin-bottom: 0.5rem;
}
.lang-content p {
  font-family: var(--fb); font-size: 0.95rem; color: var(--sh); line-height: 1.65; margin-bottom: 1rem;
}
.lang-content pre {
  border: var(--r); background: var(--pa); padding: 1rem 1.2rem; overflow-x: auto; margin-bottom: 1rem;
}
.lang-content pre code {
  font-family: var(--fb); font-size: 0.82rem; color: var(--ik); background: none; padding: 0;
}
.lang-content code {
  font-family: var(--fb); font-size: 0.85rem; padding: 0.05em 0.25em; border: var(--rl);
}
.lang-content table {
  width: 100%; border-collapse: collapse; border: var(--r); margin-bottom: 1rem;
  font-family: var(--fb); font-size: 0.85rem;
}
.lang-content th {
  font-family: var(--fl); font-size: 0.58rem; letter-spacing: 0.1em; text-transform: uppercase;
  color: var(--mi); padding: 0.5rem 0.8rem; border-bottom: var(--r); text-align: left;
}
.lang-content td { padding: 0.45rem 0.8rem; border-bottom: var(--rl); }
.lang-content tr:last-child td { border-bottom: none; }
.lang-content a { color: var(--pr); }
.lang-content a:hover { text-decoration: underline; }
.lang-content ul, .lang-content ol {
  font-family: var(--fb); font-size: 0.95rem; color: var(--sh);
  padding-left: 1.5rem; margin-bottom: 1rem;
}
.lang-content li { margin-bottom: 0.3rem; }

/* ── Responsive ────────────────────────────────────────── */
@media (max-width: 680px) {
  .hero-title { font-size: 2rem; }
  .principles-grid { grid-template-columns: 1fr; }
  .gn-card { border-right: none !important; }
  .lang-page { grid-template-columns: 1fr; }
  .lang-sidebar { border-right: none; border-bottom: var(--r); }
}
```

- [ ] **Step 2: Run zola check**

```bash
zola check
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add static/style.css
git commit -m "feat: GRAIN design system CSS — full token set, nav, patterns, components"
```

---

### Task 4: Write base.html

**Files:**
- Replace: `templates/base.html`

- [ ] **Step 1: Write templates/base.html**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{% block title %}MonaDB{% endblock title %}</title>
  <meta name="description" content="{% block description %}MonaDB — an embedded database with a query language of its own{% endblock description %}">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Playfair+Display:ital,wght@0,400;0,700;0,900;1,400;1,700&family=Courier+Prime:ital,wght@0,400;0,700;1,400&family=Space+Mono:wght@400&display=swap" rel="stylesheet">
  <link rel="stylesheet" href="{{ get_url(path='style.css') }}">
  <script src="{{ get_url(path='grain.js') }}" defer></script>
</head>
<body>
  <header class="site-nav">
    <div class="nav-inner">
      <a class="nav-brand" href="{{ get_url(path='/') }}">mona<span class="nav-dot">·</span>db</a>
      <nav class="nav-links">
        <a href="{{ get_url(path='@/language/introduction.md') }}"
          {% if current_path is containing("/language/") %}class="nav-active"{% endif %}>Language</a>
        <a href="{{ get_url(path='@/design.md') }}"
          {% if current_path is containing("/design/") %}class="nav-active"{% endif %}>Design</a>
        <a href="https://github.com/rchowell/MonaDB" class="nav-github">GitHub</a>
      </nav>
    </div>
  </header>

  {% block content %}{% endblock content %}

  <footer class="site-foot">
    <div class="foot-inner">
      <span class="foot-name">MonaDB</span>
      <a href="https://github.com/rchowell/MonaDB" class="foot-link">rchowell/MonaDB</a>
    </div>
  </footer>
</body>
</html>
```

- [ ] **Step 2: Run zola serve and open http://127.0.0.1:1111**

```bash
zola serve
```

Verify: nav renders with "mona·db" brand in Playfair, Language / Design / GitHub links in Space Mono uppercase. Footer shows "MonaDB" label and repo link.

- [ ] **Step 3: Commit**

```bash
git add templates/base.html
git commit -m "feat: base.html — GRAIN nav, footer, Google Fonts"
```

---

### Task 5: Build the landing page

**Files:**
- Replace: `templates/index.html`
- Already created: `content/_index.md` (no change needed)

- [ ] **Step 1: Write templates/index.html**

```html
{% extends "base.html" %}

{% block title %}MonaDB — A query language for embedded documents{% endblock title %}
{% block description %}MonaDB pairs a small document-friendly SQL dialect with a stack-based bytecode engine — text in, results out, no server.{% endblock description %}

{% block content %}

<section class="hero">
  <canvas class="hero-canvas" data-grain="hero" data-height="400"></canvas>
  <div class="hero-inner">
    <p class="hero-eyebrow">Embedded database</p>
    <h1 class="hero-title">A query language for embedded documents.</h1>
    <p class="hero-lead">
      RQL compiles to stack-based bytecode and runs inside your process.
      Objects, arrays, and path traversal are first-class constructs, not extensions.
      No server. No daemon. The database lives where your code does.
    </p>
    <div class="hero-cta">
      <a class="gn-btn gn-btn-pri" href="{{ get_url(path='@/language/introduction.md') }}">Read the language</a>
      <a class="gn-btn gn-btn-sec" href="{{ get_url(path='@/design.md') }}">How it works</a>
    </div>
  </div>
</section>

<section class="landing-sect">
  <div class="landing-inner">
    <p class="sect-eyebrow">Query pipeline</p>
    <h2 class="sect-title">One path from text to storage.</h2>
    <div class="pipeline">
      <span class="pipeline-step">SQL text</span>
      <span class="pipeline-step">Lexer</span>
      <span class="pipeline-step">Parser</span>
      <span class="pipeline-step">IR</span>
      <span class="pipeline-step">Compiler</span>
      <span class="pipeline-step">Vop bytecode</span>
      <span class="pipeline-step">VM</span>
      <span class="pipeline-step">LMDB</span>
    </div>
  </div>
</section>

<section class="landing-sect">
  <div class="landing-inner">
    <p class="sect-eyebrow">A taste of RQL</p>
    <h2 class="sect-title">Familiar, simplified.</h2>
    <pre class="landing-pre"><code>create table points;

insert into points (
    { x: 1, y: 2 },
    { x: 3, y: 4 },
);

select { x: p.x, y: p.y }
  from points as p
 where p.x > 1
 fetch 10;</code></pre>
  </div>
</section>

<section class="landing-sect">
  <div class="landing-inner">
    <p class="sect-eyebrow">Design principles</p>
    <h2 class="sect-title">What makes RQL different.</h2>
    <div class="principles-grid">
      <div class="gn-card">
        <canvas class="gn-card-img" data-grain="dot25" data-height="90"></canvas>
        <div class="gn-card-body">
          <h3 class="gn-card-title">Documents</h3>
          <p>Objects, arrays, and JSONPath-style traversal are part of the language model. The schema is optional — or absent entirely.</p>
        </div>
      </div>
      <div class="gn-card">
        <canvas class="gn-card-img" data-grain="dot50" data-height="90"></canvas>
        <div class="gn-card-body">
          <h3 class="gn-card-title">Bytecode</h3>
          <p>Queries compile to compact Vop instructions. The VM executes a tight loop over them rather than interpreting the tree directly.</p>
        </div>
      </div>
      <div class="gn-card">
        <canvas class="gn-card-img" data-grain="dot25" data-height="90"></canvas>
        <div class="gn-card-body">
          <h3 class="gn-card-title">Embedded</h3>
          <p>Backed by LMDB. The database runs inside your process — no server, no configuration files, no background daemon.</p>
        </div>
      </div>
      <div class="gn-card">
        <canvas class="gn-card-img" data-grain="dot50" data-height="90"></canvas>
        <div class="gn-card-body">
          <h3 class="gn-card-title">Familiar</h3>
          <p>Most of the SQL you already know: select, from, where, group, order. Fewer corners, document-friendly defaults.</p>
        </div>
      </div>
    </div>
  </div>
</section>

{% endblock content %}
```

- [ ] **Step 2: Run zola serve and verify landing at http://127.0.0.1:1111**

```bash
zola serve
```

Check:
- Hero canvas renders a halftone gradient (Paper background with Ink dots, light-to-dark left-to-right)
- Eyebrow "EMBEDDED DATABASE" in Space Mono uppercase
- Title "A query language for embedded documents." in Playfair Display 900
- Two buttons: solid Ink-fill "READ THE LANGUAGE" and outline "HOW IT WORKS"
- Pipeline strip shows 8 stages in Space Mono uppercase, separated by heavy borders
- Code block in Courier Prime, GRAIN border, Paper background
- Four cards in 2×2 grid, each with a wave-modulated halftone canvas image

- [ ] **Step 3: Commit**

```bash
git add templates/index.html
git commit -m "feat: landing page — GRAIN hero, pipeline, code taste, principle cards"
```

---

### Task 6: Build the design page

**Files:**
- Replace: `templates/page.html`
- Replace: `content/design.md`

- [ ] **Step 1: Write templates/page.html**

```html
{% extends "base.html" %}

{% block title %}{{ page.title }} — MonaDB{% endblock title %}
{% block description %}{{ page.description }}{% endblock description %}

{% block content %}
<div class="prose-page">
  {{ page.content | safe }}
</div>
{% endblock content %}
```

- [ ] **Step 2: Write content/design.md (all-new copy)**

```markdown
+++
title = "Design"
description = "How MonaDB compiles query text to bytecode and executes it against LMDB."
+++

# Design

MonaDB is a compiler and a virtual machine. A query enters as text and exits as a result set. Between those two points it passes through six stages: lexer, parser, IR, compiler, VM, and storage. Each stage has a single job and hands off a well-typed value to the next.

## Lexer

The lexer is a [logos](https://github.com/maciejhirsz/logos) DFA over the raw query string. It produces a spanned token stream — each token carries the byte range of its source characters. Spans flow forward through every subsequent stage and surface in error messages, so a type error can point at the offending expression in the original text rather than a position in the bytecode.

The token set is small. Keywords (`select`, `from`, `where`, `insert`, `into`, `create`, `table`, `update`, `set`, `delete`, `drop`, `copy`, `as`, `and`, `or`, `not`, `null`, `true`, `false`, `limit`, `fetch`, `step`, `order`, `group`, `with`, `by`, `asc`, `desc`, `default`) are distinguished from identifiers at the lexer level. All other names are identifiers.

## Parser

The parser is a [LALRPOP](https://github.com/lalrpop/lalrpop) LR(1) grammar. Grammar rules stay thin — each action constructs an IR value by calling a function in `ir.rs`. No transformation or validation happens inside the grammar. The parser's only job is shape, not meaning.

Operator precedence and associativity are declared inline on the `Expr` production using LALRPOP's `#[precedence]` and `#[assoc]` annotations. This keeps the grammar readable while avoiding ambiguity.

## IR

The IR distinguishes two kinds of types.

**Enums** for sum types — when a value is one of several alternatives. `Statement`, `Expr`, `Type`, `Fetch`, `Constructor`, `Member`, `Source`, `Selector`, and `Segment` are all enums.

**Structs** for product types — when a value groups several named fields. `Select`, `Insert`, `Create`, `Update`, `Op`, `Jpk`, `Jpi`, `Jpe`, `Iter`, and `Table` are all structs.

Recursive types are always boxed through a type alias. `ExprRef = Box<Expr>` and `TypeRef = Box<Type>` appear throughout; `Box<Expr>` inline does not. This discipline keeps the IR consistent and refactoring tractable.

## Compiler

The compiler is a tree-walking pass over the IR that emits a flat sequence of `Vop` instructions. Dispatch methods carry the `cc_` prefix — `cc_select`, `cc_expr`, `cc_insert`. Append helpers carry `emit_` — `emit_push`, `emit_jpk`, `emit_rewind`.

Control-flow instructions — `Rewind`, `IfNot`, `Next`, `CntIfPos`, `CntIfZero` — are emitted with placeholder jump targets of `0`. After the loop body is fully emitted and its size is known, the compiler back-patches the target addresses via `patch(pc, dst)`. This avoids a two-pass approach while keeping the emitter linear.

Variables are tracked by index. `define(name)` appends a `Var` entry; a variable's index into `vars` is its address on the VM stack. `Load(idx)` and `Store(idx)` address by that index. `define_counter(n)` allocates a counter slot and emits a `CntSet` instruction.

## VM

The VM is a stack-based interpreter. It maintains a value stack and a counter array, then loops over the `Vop` program dispatching each instruction. The main loop is a `match` over `Vop` variants — one arm per opcode.

Variables live on the stack as absolute-indexed slots, not in a separate frame allocation. Counters drive loop bounds: `fetch 10` compiles to a `CntSet(10)` followed by `CntIfZero(→exit)` guards around the loop body.

Cursor operations follow the pattern `Open → Scan → [body] → Next → Halt`. The cursor is always valid inside the body; `Scan` positions it, `Next` advances it, and when the source is exhausted `Next` branches to the instruction after `Halt`.

## Storage

Execution reads and writes [LMDB](http://www.lmdb.tech/doc/), a memory-mapped B+tree store. There is no server process — the engine is a library that runs inside the host program. Each table is an LMDB named database. The database name is the table's OID encoded as a fixed-width big-endian hex string.

Keys are shredded from the record using order-preserving encoding. Values are stored verbatim — the format is schemaless. Deletions are deferred: during a scan, `(cursor, key)` pairs are buffered and applied at `Halt` after the read iterator is dropped, to avoid invalidating the cursor mid-scan.
```

- [ ] **Step 3: Run zola serve, open http://127.0.0.1:1111/design/**

Check:
- Heading "Design" in Playfair Display 900 with heavy bottom rule
- Section headings (Lexer, Parser, …) in Playfair Display 700 with regular bottom rule
- Body prose in Courier Prime, Shadow color
- Inline code with Ghost light border
- "Design" nav link highlighted

- [ ] **Step 4: Commit**

```bash
git add templates/page.html content/design.md
git commit -m "feat: design page — prose template + all-new architecture copy"
```

---

### Task 7: Build the language sidebar template

**Files:**
- Replace: `templates/language/page.html`

- [ ] **Step 1: Write templates/language/page.html**

```html
{% extends "base.html" %}

{% block title %}{{ page.title }} — Language — MonaDB{% endblock title %}
{% block description %}{{ page.description }}{% endblock description %}

{% block content %}
{% set lang_section = get_section(path="language/_index.md") %}
<div class="lang-page">
  <aside class="lang-sidebar">
    <p class="sidebar-heading">Language</p>
    <ul class="sidebar-nav">
      {% for p in lang_section.pages %}
      <li>
        <a href="{{ p.permalink }}"
          {% if p.permalink == current_url %}class="active"{% endif %}>
          {{ p.title }}
        </a>
      </li>
      {% endfor %}
    </ul>
  </aside>
  <main class="lang-content">
    {{ page.content | safe }}
  </main>
</div>
{% endblock content %}
```

- [ ] **Step 2: Run zola serve, open http://127.0.0.1:1111/language/introduction/**

Check:
- Two-column layout: 200px sidebar left, content right
- Sidebar heading "LANGUAGE" in Space Mono
- Six section links in order: Introduction, Statements, Syntax, Types, Expressions, Functions
- Active page (Introduction) has Prussian left border and Ink color
- Clicking sidebar links navigates correctly, active state updates

- [ ] **Step 3: Commit**

```bash
git add templates/language/page.html
git commit -m "feat: language page template — GRAIN sidebar with active state"
```

---

### Task 8: Write all language reference content

**Files:**
- Replace: `content/language/introduction.md`
- Replace: `content/language/statements.md`
- Replace: `content/language/syntax.md`
- Replace: `content/language/types.md`
- Replace: `content/language/expressions.md`
- Replace: `content/language/functions.md`

- [ ] **Step 1: Write content/language/introduction.md**

```markdown
+++
title = "Introduction"
description = "What RQL is, how it relates to SQL, and how to read this reference."
weight = 1
+++

# Introduction

RQL is a SQL-flavored query language for embedded document storage. It keeps the clause vocabulary of standard SQL — `select`, `from`, `where`, `group`, `order`, `limit` — and extends it to treat objects, arrays, and path traversal as core constructs rather than extensions.

Every query compiles to a sequence of stack-based bytecode instructions and runs inside the host process. There is no server, no network, no configuration file. The database is a library.

## The clause model

Each clause in a query is a transform over a stream of bindings.

| Clause | Operation |
|--------|-----------|
| `from` | Iterate — produce one binding per row |
| `with` | Map — extend each binding |
| `select` | Map — construct the output value |
| `where` | Filter — drop bindings that fail the predicate |
| `group` | Reduce — collapse bindings by key |
| `order` | Sort — reorder the binding stream |
| `limit` / `fetch` | Limit — take at most N bindings, with optional offset |

The clauses compose left-to-right. `from` produces bindings; each subsequent clause transforms them; `select` maps them to output values.

## Documents

RQL treats objects and arrays as first-class values. Object literals use `{ key: value }` syntax. Arrays use `[value, value]`. Path traversal uses `$` notation rooted at a table or variable.

```
{ x: 1, y: 2 }           -- object literal
[1, 2, 3]                 -- array literal
T$.address.city           -- path into T
```

Schemas are optional. A table declared without a schema accepts any value.

## How to read this reference

The remaining sections cover the language in bottom-up order: [Syntax](/language/syntax/) and identifiers first, then [Types](/language/types/), then [Expressions](/language/expressions/), then [Statements](/language/statements/) that compose them. [Functions](/language/functions/) covers built-in functions and `read()`. Start with [Statements](/language/statements/) if you want to write queries immediately.
```

- [ ] **Step 2: Write content/language/statements.md**

```markdown
+++
title = "Statements"
description = "SELECT, INSERT, UPDATE, CREATE TABLE, DROP TABLE, and COPY."
weight = 2
+++

# Statements

A MonaDB program is a sequence of statements separated by semicolons. Each statement performs one operation: query, insert, update, create, drop, or export.

## select

Maps the current binding stream and applies a constructor — an object literal, a list of named expressions, or `*` to spread all bound variables.

```
select <constructor>
  [from <source>]
  [where <expr>]
  [group by <expr>]
  [order by <expr> [asc|desc]]
  [fetch <range>];
```

```
select 1 + 1;

select { x: p.x, y: p.y }
  from points as p;

select p.x as x, p.y as y
  from points as p
 where p.x > 0;

select * from t;          -- equivalent to { ...t }
select * from t, s;       -- { ...t, ...s }
```

## insert

`insert into <table>` followed by a parenthesised, comma-separated values list. A trailing comma is permitted.

```
insert into points ({ x: 1, y: 2 });

insert into points (
    { x: 1, y: 2 },
    { x: 3, y: 4 },
);

insert into numbers (1, 2, 3);
insert into tuples ([1, 2], [3, 4]);
```

## update

`update <table> set <col> = <expr>, ...` with an optional `where` clause. Only rows matching the predicate are updated. Column values may be an expression, `DEFAULT`, or `NULL`.

```
update points set x = 10 where x = 0;
update points set x = x + 1, y = x + 1 where y = 0;
```

## create table

A table is a collection with an optional type constraint and optional index declarations. Members are `NOT NULL` by default; append `|null` to permit null.

```
create table points;          -- no schema
create table points ();       -- equivalent

create table points ({
    x: number,
    y: number,
    z: number|null,           -- nullable
    ...                       -- open content
}, {
    hash: x,                  -- partition key
    sort: y,                  -- range key
});
```

The second block declares index keys. Both `hash` and `sort` are optional.

## drop table

Removes a table and all its contents.

```
drop table points;
```

## copy

Moves data between a table or query and a file. Format is inferred from the extension or set via the `format:` option.

```
copy items to 'items.jsonl';
copy items to 'items.csv';
copy items to 'items.tsv' { header: false };
```

Supported formats: `jsonl`, `csv`, `tsv`.
```

- [ ] **Step 3: Write content/language/syntax.md**

```markdown
+++
title = "Syntax"
description = "Identifiers, reserved words, literals, comments, and semicolons."
weight = 3
+++

# Syntax

## Identifiers

An identifier is any UTF-8 name that is not a reserved word. Identifiers are case-sensitive. A name that collides with a reserved word may be quoted with backticks.

```
points          -- valid
myTable         -- valid
`select`        -- quoted reserved word, valid
```

## Reserved words

The following words are reserved and may not appear as bare identifiers:

`and` `as` `asc` `by` `copy` `create` `default` `delete` `desc` `drop` `false` `fetch` `from` `group` `insert` `into` `limit` `not` `null` `or` `order` `select` `set` `step` `table` `true` `update` `where` `with`

## Literals

**Numbers** are decimal integers or floating-point values.

```
42      3.14      -1
```

**Strings** are single-quoted. Escape sequences: `\'` `\\` `\n` `\t`.

```
'hello'      'it\'s fine'
```

**Booleans**: `true` and `false`.

**Null**: `null`.

**Objects**: `{ key: value, ... }`. Keys are bare identifiers. A trailing comma is permitted.

**Arrays**: `[value, ...]`. A trailing comma is permitted.

## Comments

Single-line comments begin with `--` and run to end of line. Block comments are not supported.

```
select x from t;   -- this is a comment
```

## Semicolons

Statements are separated by semicolons. A trailing semicolon after the last statement is optional in the REPL and required in multi-statement programs.
```

- [ ] **Step 4: Write content/language/types.md**

```markdown
+++
title = "Types"
description = "Scalar types, collection types, aliases, and nullability."
weight = 4
+++

# Types

RQL has five base types. All have a canonical name and a short alias. Types appear in `create table` schema declarations and in cast expressions.

| Type | Alias | Description |
|------|-------|-------------|
| `boolean` | `bool` | `true` or `false` |
| `number` | `num` | 64-bit floating-point |
| `string` | `str` | UTF-8 text |
| `array` | `arr` | Ordered sequence of values |
| `object` | `obj` | Unordered map of named fields |

## Nullability

Schema members are `NOT NULL` by default. Append `|null` to a type to allow null.

```
create table readings ({
    sensor: string,
    value:  number,
    label:  string|null,   -- nullable
});
```

A nullable field accepts either a typed value or `null`. A non-nullable field rejects `null` at insert time.

## Open content

Append `...` as the last member of an object schema to allow extra fields. Without `...`, inserting an object with undeclared fields is a type error.

```
create table events ({
    id:   number,
    name: string,
    ...               -- any extra fields allowed
});
```

## No schema

A table declared as `create table t;` accepts any value. Type checking is skipped entirely.
```

- [ ] **Step 5: Write content/language/expressions.md**

```markdown
+++
title = "Expressions"
description = "Operators, precedence, object constructors, path traversal, and type coercion."
weight = 5
+++

# Expressions

## Operators

| Category | Operators |
|----------|-----------|
| Arithmetic | `+`  `-`  `*`  `/`  `%` |
| Comparison | `=`  `!=`  `<`  `>`  `<=`  `>=` |
| Logical | `and`  `or`  `not` |
| String | `\|\|` (concatenation) |

Operator precedence, highest to lowest:

1. Unary `-`, `not`
2. `*`  `/`  `%`
3. `+`  `-`  `||`
4. `=`  `!=`  `<`  `>`  `<=`  `>=`
5. `and`
6. `or`

All binary operators are left-associative at the same level.

## Object constructors

```
{ a: 1, b: 2 }
{ ...t, extra: true }    -- spread t, add extra
{ ...a, ...b }           -- merge (b wins on conflict)
{ x, y }                 -- shorthand for { x: x, y: y }
```

A trailing comma is permitted.

## Array constructors

```
[1, 2, 3]
[x, y, x + y]
```

## Path traversal

Path traversal is rooted at a table or variable with `$`. Single field access collapses to the value. Use in `from` to iterate a nested collection.

```
T$.address              -- the address field of T
T$.tags[0]              -- first element of tags
T$['key']               -- bracket notation, equivalent to T$.key
T$[x, y]                -- select fields x and y

from T$.items as item   -- iterate the items array of each row
```

## Cast and coercion

Three interchangeable forms:

```
cast(v as bool)
v::bool
bool(v)
```

| From | → bool | → number | → string |
|------|--------|----------|----------|
| `0` | `false` | — | `'0'` |
| `''` | `false` | — | — |
| `[]` | `false` | — | — |
| `true` | — | `1` | `'true'` |
| `false` | — | `0` | `'false'` |
| `'3.14'` | `true` | `3.14` | — |
| `null` | `false` | `0` | `'null'` |
```

- [ ] **Step 6: Write content/language/functions.md**

```markdown
+++
title = "Functions"
description = "Function call syntax, aggregate functions, string functions, and read()."
weight = 6
+++

# Functions

## Call syntax

Functions are called by name with positional arguments. Named arguments use `name: value` syntax and may follow positional arguments in any order.

```
upper('hello')
round(3.14159, 2)
read('data.jsonl', format: 'jsonl')
```

## Aggregate functions

Aggregate functions reduce a group of rows to a single value. They are valid only inside a `select` with a `group by` clause, or as the outermost expression in a scalar `select`.

| Function | Description |
|----------|-------------|
| `count(*)` | Number of rows in the group |
| `count(expr)` | Number of non-null values |
| `sum(expr)` | Sum of numeric values |
| `avg(expr)` | Average of numeric values |
| `min(expr)` | Minimum value |
| `max(expr)` | Maximum value |

```
select { tag: tag, n: count(*) }
  from items
 group by tag;
```

## String functions

| Function | Description |
|----------|-------------|
| `upper(str)` | Uppercase |
| `lower(str)` | Lowercase |
| `length(str)` | Character count |
| `trim(str)` | Strip leading and trailing whitespace |
| `substr(str, start, len)` | Substring by character offset |
| `contains(str, substr)` | Returns `true` if str contains substr |

## read()

`read(path)` reads a file and returns its contents as a value or row sequence for use in `from`. Format is inferred from the file extension; override with the `format:` named argument.

```
select * from read('data.jsonl') as row;
select * from read('records.csv', header: true) as row;
```

Supported formats: `jsonl`, `csv`, `tsv`.
```

- [ ] **Step 7: Run zola serve and spot-check all six pages**

```bash
zola serve
```

Open in sequence:
- `http://127.0.0.1:1111/language/introduction/`
- `http://127.0.0.1:1111/language/statements/`
- `http://127.0.0.1:1111/language/syntax/`
- `http://127.0.0.1:1111/language/types/`
- `http://127.0.0.1:1111/language/expressions/`
- `http://127.0.0.1:1111/language/functions/`

Verify for each: sidebar present, active item highlighted with Prussian border, tables render with GRAIN header style, code blocks border correctly.

- [ ] **Step 8: Run zola check**

```bash
zola check
```

Expected: no broken internal links, no template errors.

- [ ] **Step 9: Commit**

```bash
git add content/language/
git commit -m "feat: language reference — six sections, all-new RQL copy"
```

---

### Task 9: Update GitHub Actions workflow

**Files:**
- Replace: `.github/workflows/pages.yml`

- [ ] **Step 1: Replace .github/workflows/pages.yml**

```yaml
name: Deploy Pages

on:
  push:
    branches: [main]
    paths:
      - "content/**"
      - "templates/**"
      - "static/**"
      - "config.toml"
      - ".github/workflows/pages.yml"
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Zola
        run: |
          wget -q https://github.com/getzola/zola/releases/download/v0.19.2/zola-v0.19.2-x86_64-unknown-linux-gnu.tar.gz
          tar xf zola-v0.19.2-x86_64-unknown-linux-gnu.tar.gz
          sudo mv zola /usr/local/bin/zola

      - name: Build
        run: zola build

      - uses: actions/configure-pages@v5

      - uses: actions/upload-pages-artifact@v3
        with:
          path: public

  deploy:
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - uses: actions/deploy-pages@v4
        id: deployment
```

> **Note:** Check https://github.com/getzola/zola/releases for the latest stable release and update the version string in the `wget` URL if a newer version is available.

- [ ] **Step 2: Run zola build locally to confirm it passes**

```bash
zola build
```

Expected: creates `public/` directory, ends with `Done in X ms.`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/pages.yml
git commit -m "feat: GitHub Actions — Zola build + GitHub Pages deploy"
```

---

### Task 10: Remove legacy site/, final check, deploy

**Files:**
- Delete: `site/` directory

- [ ] **Step 1: Remove the old site directory**

```bash
git rm -r site/
git commit -m "chore: remove legacy site/ directory"
```

- [ ] **Step 2: Run zola check**

```bash
zola check
```

Expected: no errors.

- [ ] **Step 3: Inspect build output**

```bash
zola build && ls public/
```

Expected files present in `public/`:
```
index.html
design/index.html
language/introduction/index.html
language/statements/index.html
language/syntax/index.html
language/types/index.html
language/expressions/index.html
language/functions/index.html
style.css
grain.js
sitemap.xml
robots.txt
```

- [ ] **Step 4: Push and verify deploy**

```bash
git push origin main
```

Open the GitHub Actions tab. Watch the "Deploy Pages" workflow run to completion. Open the deployed URL (`https://rchowell.github.io/MonaDB`) and verify:

- Landing: halftone hero canvas, Space Mono pipeline strip, Courier Prime code block, Playfair card titles, `gn-btn-pri` / `gn-btn-sec` buttons
- `/design/`: prose in GRAIN type scale, Heavy border under h1, Regular borders under h2
- `/language/introduction/`: sidebar with Prussian active indicator, full two-column layout
- Nav active states update correctly on each page
- GitHub link goes to `https://github.com/rchowell/MonaDB`

---

## Self-review

| Spec requirement | Task |
|---|---|
| Full Zola approach (all pages through templates/content) | Tasks 1, 4–8 |
| GRAIN color tokens (all 6 values + 3 border weights) | Task 3 |
| Google Fonts: Playfair Display, Courier Prime, Space Mono | Task 4 |
| GRAIN pattern classes (ht10–htcross) | Task 3 |
| grain.js: halftoneGradient + halftoneCard | Task 2 |
| No smooth CSS gradients in UI, no blur shadows | Task 3 (CSS has none) |
| Landing hero with canvas halftone | Task 5 |
| Landing pipeline strip (Space Mono, bordered) | Task 5 |
| Landing code taste | Task 5 |
| Landing principle cards with canvas images | Task 5 |
| Design page: all-new prose copy, 6 sections | Task 6 |
| Language sidebar built from get_section() | Task 7 |
| Sidebar active state via Prussian border-left | Task 7 |
| 6 language sections: all-new copy | Task 8 |
| Language URL: /language/\<section\>/ | Tasks 1, 7–8 |
| Design URL: /design/ | Tasks 1, 6 |
| GitHub Actions: Zola build + Pages deploy | Task 9 |
| Old site/ removed | Task 10 |
| public/ in .gitignore | Task 1 |
