# MonaDB Design Language

A print-inspired visual language built on halftone, dithering, and the hard-won restraint of the ink press.

---

## Philosophy

GRAIN is designed for surfaces that refuse the frictionless — most directly, e-ink displays, but any medium where depth must be earned through structure rather than light. Four principles govern every decision.

**Pixels are a resource.** Every rendered element earns its place. No smooth gradient does what a halftone pattern cannot do more honestly. Tonal range is expressed through density, not opacity. Smooth transitions are replaced by visible thresholds.

**Pattern is the palette.** Color is scarce; pattern is abundant. Six ink values replace an infinite ramp. The dot, the line, the cross — these are the material of the design. Decoration emerges from structure, never from addition.

**Type carries ink.** Typefaces are chosen for their print heritage — optical weight, ink traps, the feel of letterpress. No screen-optimized grotesques. Type should look like it was made to be pressed into paper.

**No shadows. No glow.** E-ink displays have no backlight. Depth is communicated through borders, weight, and pattern density — never through drop shadows, blurs, or gradients. The surface is flat. The information is not.

---

## Color System

The palette contains exactly six values, each with a named role. These are not interchangeable — every token exists to do a specific job, and the system breaks if tokens are swapped for convenience.

| Token    | Name     | Hex       | CSS Variable | Role                                        |
|----------|----------|-----------|--------------|---------------------------------------------|
| Paper    | Paper    | `#EBE7D8` | `--pa`       | Page background; the default surface        |
| Ink      | Ink      | `#1C1A14` | `--ik`       | Primary text; borders; filled elements      |
| Ghost    | Ghost    | `#C8C3B5` | `--gh`       | Dividers; light rules; disabled states      |
| Mid      | Mid      | `#8E8979` | `--mi`       | Labels; metadata; eyebrows; captions        |
| Shadow   | Shadow   | `#454039` | `--sh`       | Secondary body text; muted content          |
| Sage     | Sage     | `#4A5245` | `--pr`       | Single accent; focus states; accent buttons |

The Paper/Ink pair is the axis of the entire system. All other values exist to modulate between them. Sage (`--pr`) is the only chromatic departure and should be used sparingly: focus rings, accent tags, a variant button, a blockquote border. Using it more than once in a composition is likely a mistake.

### CSS Custom Properties

```css
#root {
  --pa: #EBE7D8;  /* Paper     — background */
  --ik: #1C1A14;  /* Ink       — primary    */
  --gh: #C8C3B5;  /* Ghost     — divider    */
  --mi: #8E8979;  /* Mid       — labels     */
  --sh: #454039;  /* Shadow    — secondary  */
  --pr: #4A5245;  /* Sage      — accent     */
}
```

### Border Tokens

Three border weights encode hierarchy without introducing new colors. All use Ink.

```css
--r:  1.5px solid #1C1A14;  /* Regular — default section rule   */
--rh: 3px   solid #1C1A14;  /* Heavy   — top cap, blockquote bar */
--rl: 1px   solid #C8C3B5;  /* Light   — cell rules, sub-dividers */
```

The heavy border (`--rh`) marks top edges of principles and blockquotes — it signals "entry point." The light border (`--rl`) separates items within a component — cards, table rows, aside items — without competing with the structural rules around it.

---

## Pattern Library

Pattern replaces tonal washes. There are no gradients in the GRAIN system except as canvas-rendered halftone ramps. All static patterns are pure CSS `background-image` constructions using `radial-gradient` and `repeating-linear-gradient`. They tile seamlessly and render crisply on high-density screens.

### Dot Halftone Series

The dot series encodes four tonal densities. The grid pitch decreases as density increases, so the physical dot size stays proportionally readable at each level.

```css
/* 10% fill — light suggestion; texture without weight */
.ht10 {
  background: #EBE7D8;
  background-image: radial-gradient(circle, #1C1A14 13%, transparent 13%);
  background-size: 9px 9px;
}

/* 25% fill — light-mid; card image texture, muted backgrounds */
.ht25 {
  background: #EBE7D8;
  background-image: radial-gradient(circle, #1C1A14 27%, transparent 27%);
  background-size: 7px 7px;
}

/* 50% fill — true mid-tone; the visual centre of the system */
.ht50 {
  background: #EBE7D8;
  background-image: radial-gradient(circle, #1C1A14 45%, transparent 45%);
  background-size: 6px 6px;
}

/* 75% fill — heavy; near-opaque; use for strong emphasis only */
.ht75 {
  background: #EBE7D8;
  background-image: radial-gradient(circle, #1C1A14 65%, transparent 65%);
  background-size: 5px 5px;
}
```

### Line Series

Line patterns add directionality and are suitable for backgrounds requiring more texture than the dot series provides without the weight.

```css
/* Diagonal — 45°, 1px stroke, 5px gap */
.htdiag {
  background: #EBE7D8;
  background-image: repeating-linear-gradient(
    45deg, #1C1A14 0, #1C1A14 1px, transparent 1px, transparent 5px
  );
}

/* Horizontal — 0°, 1px stroke, 5px gap */
.hthline {
  background: #EBE7D8;
  background-image: repeating-linear-gradient(
    0deg, #1C1A14 0, #1C1A14 1px, transparent 1px, transparent 5px
  );
}

/* Crosshatch — 0° and 90° overlaid, 5px grid */
.htcross {
  background: #EBE7D8;
  background-image:
    repeating-linear-gradient(0deg,  #1C1A14 0, #1C1A14 1px, transparent 0, transparent 5px),
    repeating-linear-gradient(90deg, #1C1A14 0, #1C1A14 1px, transparent 0, transparent 5px);
  background-size: 5px 5px;
}
```

### Accent Dot

The single pattern variant using Sage instead of Ink. Use only where the accent color is contextually established — an accent-colored border or element must be nearby.

```css
/* Sage dot — 35% fill, 6px grid */
.htaccent {
  background: #EBE7D8;
  background-image: radial-gradient(circle, #4A5245 35%, transparent 35%);
  background-size: 6px 6px;
}
```

### Canvas-Rendered Halftone Gradient

Continuous-tone halftone ramps (like the hero banner) cannot be expressed in pure CSS — they require canvas. The rendering algorithm samples luminosity at each grid position and draws a dot whose radius is proportional to the square root of the local tone value. Using square root (rather than linear) radius produces perceptually even density progression.

```js
function halftoneGradient(ctx, W, H, invert) {
  const paper = [235, 231, 216], ink = [28, 26, 20];
  ctx.fillStyle = `rgb(${paper})`; ctx.fillRect(0, 0, W, H);
  ctx.fillStyle = `rgb(${ink})`;
  const g = 7; // grid pitch in px
  for (let y = g / 2; y < H; y += g) {
    for (let x = g / 2; x < W; x += g) {
      const raw = x / W;
      const d = invert ? 1 - raw : raw;
      const r = (g / 2 - 0.5) * Math.sqrt(d);
      if (r > 0.2) {
        ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.fill();
      }
    }
  }
}
```

The `invert` parameter lets you produce both a light-to-dark ramp (left-to-right: `false`) and a dark-to-light ramp (`true`) from the same function. The `0.2` threshold suppresses near-invisible dots at the lightest end.

For canvas-rendered text treated as a halftone mask (as in the hero logotype), the technique renders text to an offscreen canvas, reads pixel luminosity via `getImageData`, and draws proportionally-sized dots into the visible canvas at each grid position. Grid pitch of 5px produces a readable, visible-dot effect for large display text.

---

## Typography

Three typefaces, each with a single assigned role. Mixing them outside their roles breaks the print-register the system depends on.

| Role    | Family           | Fallback               | CSS Variable |
|---------|------------------|------------------------|--------------|
| Display | Playfair Display | Georgia, serif         | `--fd`       |
| Body    | Courier Prime    | Courier New, monospace | `--fb`       |
| Label   | Space Mono       | Courier New, monospace | `--fl`       |

```css
--fd: 'Playfair Display', Georgia, serif;
--fb: 'Courier Prime', 'Courier New', monospace;
--fl: 'Space Mono', 'Courier New', monospace;
```

**Playfair Display** (`--fd`) is used for all display text — logotypes, headings, article titles, blockquote body, and anywhere the content should feel like it belongs in a masthead or broadsheet. It carries optical weight well at all sizes and has a strong italic cut suited for taglines and pull quotes.

**Courier Prime** (`--fb`) is the body text typeface. It is a screen-optimized redrawing of Courier with corrected ink traps and metrics. It should be used for all reading-length prose.

**Space Mono** (`--fl`) is the label typeface. It is always set in uppercase with wide letter-spacing for short strings — navigation links, category eyebrows, metadata, button labels, swatch names, and section numbers. Never use it for body copy.

### Type Scale

| Role          | Font             | Weight | Size    | Line-height |
|---------------|------------------|--------|---------|-------------|
| Display / 9xl | Playfair Display | 900    | 2.8rem  | 1.0         |
| Heading / 2xl | Playfair Display | 700    | 1.8rem  | 1.1         |
| Title / xl    | Playfair Display | 700    | 1.25rem | —           |
| Body / base   | Courier Prime    | 400    | 0.95rem | 1.5         |
| Caption / sm  | Courier Prime    | 400    | 0.75rem | —           |
| Eyebrow / xs  | Space Mono       | 400    | 0.6rem  | —           |

Eyebrow text is always `text-transform: uppercase` with `letter-spacing: 0.14em`. Section numbers use `0.1em` tracking at `0.58rem`. Navigation links use `0.12em` tracking at `0.6rem`. These are fixed values, not suggestions — the compressed, high-tracking label register is what gives the system its editorial character.

Base document font: `--fb` at 14px, line-height 1.65.

---

## Components

### Buttons

Four button variants cover all interactive states. All share the same padding, border, and label typography — only fill and color differ.

```css
/* Base — shared by all button variants */
.gn-btn {
  font-family: var(--fl);
  font-size: 0.62rem;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  padding: 0.45rem 1.1rem;
  border: var(--r);
  background: none;
  color: var(--ik);
  cursor: pointer;
  display: inline-block;
}

/* Primary — Ink fill, Paper text */
.gn-btn-pri { background: #1C1A14; color: #EBE7D8; }

/* Secondary — transparent fill, Ink border */
.gn-btn-sec { background: transparent; color: #1C1A14; }

/* Halftone — dot-25 fill, Ink text */
.gn-btn-ht {
  color: #1C1A14;
  background: #EBE7D8;
  background-image: radial-gradient(circle, #1C1A14 24%, transparent 24%);
  background-size: 6px 6px;
}

/* Accent — Sage fill, Paper text */
.gn-btn-acc { background: #4A5245; color: #EBE7D8; border-color: #4A5245; }
```

The Halftone button is the most distinctive variant — use it where you want a filled button that doesn't fully commit to the Ink-on-Paper weight of Primary.

### Tags and Badges

Tags share the label typography of buttons at a smaller scale. Four variants mirror the button family.

```css
.gn-tag {
  font-family: var(--fl);
  font-size: 0.58rem;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  padding: 0.18rem 0.55rem;
  border: var(--r);
  display: inline-block;
}

.gn-tag-solid  { background: #1C1A14; color: #EBE7D8; border-color: #1C1A14; }
.gn-tag-ht     { background-image: radial-gradient(circle, #1C1A14 24%, transparent 24%);
                  background-size: 5px 5px; }
.gn-tag-acc    { border-color: #4A5245; color: #4A5245; }
/* default .gn-tag alone = Outline variant */
```

### Article Card

Cards have a hard border (`--r`), a canvas-rendered halftone image area, and a footer rule (`--rl`). The title is set in Playfair Display Bold; category and footer metadata use Space Mono at the smallest label size.

```css
.gn-card         { border: var(--r); }
.gn-card-img     { width: 100%; height: 110px; display: block; } /* canvas element */
.gn-card-body    { padding: 0.9rem; }
.gn-card-cat     { font-family: var(--fl); font-size: 0.56rem; letter-spacing: 0.12em;
                   text-transform: uppercase; color: var(--mi); margin-bottom: 0.4rem; }
.gn-card-title   { font-family: var(--fd); font-size: 1rem; font-weight: 700;
                   line-height: 1.25; margin-bottom: 0.4rem; }
.gn-card-body p  { font-size: 0.78rem; color: var(--sh); line-height: 1.5; }
.gn-card-foot    { display: flex; justify-content: space-between;
                   padding: 0.6rem 0.9rem; border-top: var(--rl);
                   font-family: var(--fl); font-size: 0.56rem; color: var(--mi); }
```

The card image area should always use a canvas element with a halftone-rendered fill, not a flat color or a photographic `<img>`. The appropriate density for card images is Dot 25% or Dot 50% — use the wave-modulated variant to avoid mechanical repetition:

```js
function halftoneCardImg(ctx, W, H, type) {
  const g = type === 'dot50' ? 6 : 7;
  const pct = type === 'dot50' ? 0.45 : 0.27;
  for (let y = g / 2; y < H; y += g) {
    for (let x = g / 2; x < W; x += g) {
      const wave = Math.sin((x / W) * Math.PI) * 0.7 + Math.sin((y / H) * Math.PI * 2) * 0.15;
      const d = pct * (0.4 + wave * 0.6);
      const r = (g / 2 - 0.3) * Math.sqrt(Math.max(0, d / pct));
      if (r > 0.3) { ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2); ctx.fill(); }
    }
  }
}
```

### Form Inputs

Inputs use the body typeface at a slightly reduced size and share the paper background. Focus state shifts the border to Sage and adds a 2px solid offset shadow — the only place in the system where a shadow-like effect appears, and it is a hard 2D offset, not a blur.

```css
.gn-lbl {
  font-family: var(--fl); font-size: 0.58rem; letter-spacing: 0.1em;
  text-transform: uppercase; display: block; margin-bottom: 0.35rem;
}
.gn-input {
  font-family: var(--fb); font-size: 0.82rem;
  background: var(--pa); color: var(--ik);
  border: var(--r); padding: 0.4rem 0.65rem;
  width: 100%; outline: none; display: block;
}
.gn-input:focus {
  box-shadow: 2px 2px 0 var(--pr);
  border-color: var(--pr);
}
```

### Blockquote

The blockquote uses a 3px heavy left border and Playfair Display Italic body — the only component in the system that combines the heavy rule weight with an italic serif. The citation below is Space Mono uppercase at the smallest label size.

```css
.gn-bq         { border-left: var(--rh); padding-left: 1.2rem; }
.gn-bq p       { font-family: var(--fd); font-style: italic; font-size: 0.95rem; line-height: 1.5; }
.gn-bq cite    { font-family: var(--fl); font-size: 0.56rem; letter-spacing: 0.1em;
                 text-transform: uppercase; color: var(--mi); }
```

The Sage variant of the blockquote replaces the border-left color with `--pr`. This signals an "editorial aside" reading as opposed to a direct quotation. Nothing else changes.

---

## Layout and Spacing

The layout is a single-column document flow divided by full-width section rules. Sections use a consistent padding of `2.5rem 2rem`. Section headers are `display: flex; align-items: baseline` with a section number (Space Mono, 0.58rem, `--mi`) and a title (Playfair Display Bold, 1.3rem). The header itself carries a bottom rule (`--r`) and a bottom margin of `2rem`.

```css
.gn-sect    { padding: 2.5rem 2rem; border-bottom: var(--r); }
.gn-sect-hd { display: flex; align-items: baseline; gap: 1rem;
              margin-bottom: 2rem; padding-bottom: 0.75rem; border-bottom: var(--r); }
.gn-sect-num   { font-family: var(--fl); font-size: 0.58rem; letter-spacing: 0.1em; color: var(--mi); }
.gn-sect-title { font-family: var(--fd); font-size: 1.3rem; font-weight: 700; }
```

Internal grids use two standard column configurations: a 2-column grid for principle cards and component pairs (`1fr 1fr`, gap `2rem`), and a 4-column grid for the pattern library (`repeat(4, 1fr)`) with Ghost light rules between cells. Color swatches use a 6-column grid with Ghost right-borders.

The navigation is sticky at `top: 0` with the Paper background, using `z-index: 10`. Logo: Playfair Display Black, 1.05rem, `letter-spacing: 0.18em`. Links: Space Mono uppercase, 0.6rem, `letter-spacing: 0.12em`, no decoration, underline on hover.

---

## Constraints and Anti-Patterns

The following are explicit violations of the system, not stylistic preferences.

**No drop shadows or blurs.** `box-shadow` with a blur radius, `filter: blur()`, and `text-shadow` are all forbidden. The only permitted shadow-like effect is the 2px hard offset on focused inputs, which is a structural offset, not a diffusion.

**No smooth gradients in UI elements.** CSS `linear-gradient` and `radial-gradient` may only appear as the halftone dot and line patterns specified in the Pattern Library. Smooth tonal ramps in visual areas (hero, card images) must be canvas-rendered using the halftone algorithm.

**No screen-optimized grotesques.** Inter, SF Pro, Roboto, and their category are not part of this system. The label role must be filled by Space Mono; the body role by Courier Prime.

**No unearned accent color.** Sage (`--pr`) must not be used more than once per composition unless that composition has a clearly established accent axis (e.g., a Sage blockquote border next to a Sage tag). Using it for hover states across all interactive elements is a violation.

**No loose text nodes.** All text must be in an element that explicitly sets its font-family. Inheriting a generic body font and overriding selectively leads to regressions. Every semantic zone declares its typeface.

