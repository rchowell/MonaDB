/* GRAIN runtime — halftone canvases, console tabs, copy buttons.
   Vanilla JS, no framework. Ink dots on bone, the print stays honest. */
(function () {
  'use strict';

  var PAGE = '#F2F2F2';   // Bone
  var INK = '#1C1A14';    // Ink
  var ACCENT = '#F2DF2A'; // Marker

  /* ── Canvas helpers ───────────────────────────────────── */
  function ctxFor(canvas, w, h) {
    var dpr = window.devicePixelRatio || 1;
    canvas.width = Math.max(1, Math.round(w * dpr));
    canvas.height = Math.max(1, Math.round(h * dpr));
    var ctx = canvas.getContext('2d');
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    return ctx;
  }

  // Wordmark rendered as a halftone field: draw text to an offscreen
  // buffer, then stamp a dot per 5px cell sized by that cell's darkness.
  function drawWordmark(canvas, text) {
    var w = canvas.offsetWidth || (canvas.parentElement ? canvas.parentElement.offsetWidth : 0);
    var h = canvas.offsetHeight || 150;
    if (!w) return;
    var dpr = window.devicePixelRatio || 1;
    var Wd = Math.round(w * dpr), Hd = Math.round(h * dpr);

    var off = document.createElement('canvas');
    off.width = Wd; off.height = Hd;
    var oc = off.getContext('2d');
    oc.setTransform(dpr, 0, 0, dpr, 0, 0);
    oc.fillStyle = PAGE; oc.fillRect(0, 0, w, h);
    oc.fillStyle = INK;
    var fs = Math.min(h * 0.94, w * 0.168);
    oc.font = '900 ' + fs + "px 'Playfair Display', Georgia, serif";
    oc.textAlign = 'center'; oc.textBaseline = 'middle';
    oc.fillText(text, w / 2, h / 2 + fs * 0.02);
    var img = oc.getImageData(0, 0, Wd, Hd).data;

    var ctx = ctxFor(canvas, w, h);
    ctx.fillStyle = PAGE; ctx.fillRect(0, 0, w, h);
    ctx.fillStyle = INK;
    var grid = 5;
    for (var y = 0; y < h; y += grid) {
      for (var x = 0; x < w; x += grid) {
        var sx = Math.min(Math.round((x + grid / 2) * dpr), Wd - 1);
        var sy = Math.min(Math.round((y + grid / 2) * dpr), Hd - 1);
        var dark = 1 - img[(sy * Wd + sx) * 4] / 255;
        if (dark > 0.08) {
          var r = (grid / 2 - 0.4) * Math.sqrt(dark);
          ctx.beginPath();
          ctx.arc(x + grid / 2, y + grid / 2, r, 0, Math.PI * 2);
          ctx.fill();
        }
      }
    }
  }

  // Left-to-right halftone gradient in the accent ink (the console viz).
  function drawViz(canvas) {
    var w = canvas.offsetWidth, h = canvas.offsetHeight || 56;
    if (!w) return;
    var ctx = ctxFor(canvas, w, h);
    ctx.fillStyle = PAGE; ctx.fillRect(0, 0, w, h);
    ctx.fillStyle = ACCENT;
    var g = 6;
    for (var y = g / 2; y < h; y += g) {
      for (var x = g / 2; x < w; x += g) {
        var r = (g / 2 - 0.5) * Math.sqrt(x / w);
        if (r > 0.2) {
          ctx.beginPath();
          ctx.arc(x, y, r, 0, Math.PI * 2);
          ctx.fill();
        }
      }
    }
  }

  function drawAll() {
    document.querySelectorAll('canvas[data-grain="wordmark"]').forEach(function (c) {
      drawWordmark(c, c.dataset.text || 'MONADB');
    });
    document.querySelectorAll('canvas[data-grain="viz"]').forEach(drawViz);
  }

  /* ── Console tabs ─────────────────────────────────────── */
  // Each program is a list of lines; each line a list of [text, tokenClass].
  var KW = 'tok-kw', COM = 'tok-com', LIT = 'tok-lit', TXT = 'tok-txt';
  var PROGRAMS = [
    [ // Select
      [['# select: filter and project a document stream', COM]],
      [['import', KW], [' monadb', TXT]],
      [['db = monadb.', TXT], ['connect', KW], ['(', TXT], ['"data.mona"', LIT], [')', TXT]],
      [['rows = db.', TXT], ['sql', KW], ['(', TXT]],
      [['    "select {x, y} from points where x > 1 fetch 10"', LIT]],
      [[').', TXT], ['fetchall', KW], ['()', TXT]]
    ],
    [ // Insert
      [['# insert: add object literals to a collection', COM]],
      [['import', KW], [' monadb', TXT]],
      [['db = monadb.', TXT], ['connect', KW], ['(', TXT], ['"data.mona"', LIT], [')', TXT]],
      [['db.', TXT], ['execute', KW], ['(', TXT]],
      [['    "insert into points ({x:1, y:2}, {x:3, y:4})"', LIT]],
      [[')', TXT]]
    ],
    [ // Create
      [['# create: optional schema, nullable with |null', COM]],
      [['import', KW], [' monadb', TXT]],
      [['db = monadb.', TXT], ['connect', KW], ['(', TXT], ['"data.mona"', LIT], [')', TXT]],
      [['db.', TXT], ['execute', KW], ['(', TXT]],
      [['    "create table points ({x:number, z:number|null})"', LIT]],
      [[')', TXT]]
    ],
    [ // Path
      [['# path: traverse nested documents with $ syntax', COM]],
      [['import', KW], [' monadb', TXT]],
      [['db = monadb.', TXT], ['connect', KW], ['(', TXT], ['"data.mona"', LIT], [')', TXT]],
      [['rows = db.', TXT], ['sql', KW], ['(', TXT]],
      [['    "select T$.address.city from users as T"', LIT]],
      [[').', TXT], ['fetchall', KW], ['()', TXT]]
    ]
  ];

  function renderProgram(pre, idx) {
    var lines = PROGRAMS[idx] || PROGRAMS[0];
    pre.textContent = '';
    lines.forEach(function (segs, li) {
      segs.forEach(function (s) {
        var span = document.createElement('span');
        span.className = s[1];
        span.textContent = s[0];
        pre.appendChild(span);
      });
      if (li < lines.length - 1) pre.appendChild(document.createTextNode('\n'));
    });
  }

  function initConsole(root) {
    var pre = root.querySelector('[data-code]');
    var tabs = Array.prototype.slice.call(root.querySelectorAll('[data-tab]'));
    if (!pre || !tabs.length) return;
    function select(idx) {
      renderProgram(pre, idx);
      tabs.forEach(function (t, i) { t.classList.toggle('is-active', i === idx); });
    }
    tabs.forEach(function (t, i) {
      t.addEventListener('click', function () { select(i); });
    });
    var start = tabs.findIndex(function (t) { return t.classList.contains('is-active'); });
    select(start < 0 ? 0 : start);
  }

  /* ── Copy page markdown ───────────────────────────────── */
  function stripFrontMatter(text) {
    if (text.slice(0, 3) !== '+++') return text;
    var end = text.indexOf('+++', 3);
    if (end === -1) return text;
    return text.slice(end + 3).replace(/^\s+/, '');
  }

  function initCopyMarkdown() {
    var btn = document.querySelector('[data-copy-markdown]');
    var src = document.getElementById('page-markdown');
    if (!btn || !src) return;
    var label = btn.querySelector('[data-copy-md-label]');
    var original = label ? label.textContent : '';
    var resetTimer;
    btn.addEventListener('click', function () {
      var raw;
      try { raw = JSON.parse(src.textContent); } catch (e) { return; }
      var text = stripFrontMatter(raw);
      try { if (navigator.clipboard) navigator.clipboard.writeText(text); } catch (e) {}
      if (label) {
        label.textContent = 'copied ✓';
        clearTimeout(resetTimer);
        resetTimer = setTimeout(function () { label.textContent = original; }, 1600);
      }
    });
  }

  /* ── Copy-to-clipboard ────────────────────────────────── */
  function initCopy(el) {
    var text = el.dataset.copy;
    var label = el.querySelector('[data-copy-label]');
    var original = label ? label.textContent : '';
    var resetTimer;
    el.addEventListener('click', function () {
      try { if (navigator.clipboard) navigator.clipboard.writeText(text); } catch (e) {}
      if (label) {
        label.textContent = 'copied ✓';
        clearTimeout(resetTimer);
        resetTimer = setTimeout(function () { label.textContent = original; }, 1600);
      }
    });
  }

  /* ── Prose code highlighting ────────────────────────── */
  var KEYWORDS = {
    and: 1, as: 1, asc: 1, by: 1, cast: 1, copy: 1, create: 1, default: 1,
    delete: 1, desc: 1, drop: 1, false: 1, fetch: 1, from: 1, group: 1,
    insert: 1, into: 1, limit: 1, not: 1, null: 1, or: 1, order: 1, read: 1,
    select: 1, set: 1, step: 1, table: 1, true: 1, update: 1, where: 1, with: 1
  };

  function appendTok(parent, text, cls) {
    if (!text) return;
    var span = document.createElement('span');
    span.className = cls;
    span.textContent = text;
    parent.appendChild(span);
  }

  function isIdentStart(ch) {
    return /[A-Za-z_$`]/.test(ch);
  }

  function isIdentPart(ch) {
    return /[A-Za-z0-9_$`]/.test(ch);
  }

  function tokenizeLine(line) {
    var tokens = [];
    var i = 0;
    while (i < line.length) {
      var ch = line[i];
      var rest = line.slice(i);

      if (rest.slice(0, 2) === '--') {
        tokens.push([line.slice(i), COM]);
        break;
      }

      if (ch === '"' || ch === "'") {
        var j = i + 1;
        while (j < line.length) {
          if (line[j] === '\\' && j + 1 < line.length) { j += 2; continue; }
          if (line[j] === ch) { j++; break; }
          j++;
        }
        tokens.push([line.slice(i, j), LIT]);
        i = j;
        continue;
      }

      if (/[0-9]/.test(ch) || (ch === '.' && i + 1 < line.length && /[0-9]/.test(line[i + 1]))) {
        var k = i + (/[0-9]/.test(ch) ? 1 : 2);
        while (k < line.length && /[0-9.]/.test(line[k])) k++;
        tokens.push([line.slice(i, k), LIT]);
        i = k;
        continue;
      }

      if (isIdentStart(ch)) {
        var m = i + 1;
        while (m < line.length && isIdentPart(line[m])) m++;
        var word = line.slice(i, m);
        var key = word.replace(/^`|`$/g, '').toLowerCase();
        tokens.push([word, KEYWORDS[key] ? KW : TXT]);
        i = m;
        continue;
      }

      tokens.push([ch, TXT]);
      i++;
    }
    return tokens;
  }

  function highlightCodeBlock(code) {
    if (code.dataset.highlighted) return;
    if (code.closest('[data-code]')) return;

    var text = code.textContent.replace(/\n$/, '');
    var lines = text.split('\n');
    code.textContent = '';
    code.dataset.highlighted = 'true';

    lines.forEach(function (line, li) {
      tokenizeLine(line).forEach(function (tok) {
        if (tok[1] === KW) appendTok(code, tok[0], KW);
        else code.appendChild(document.createTextNode(tok[0]));
      });
      if (li < lines.length - 1) code.appendChild(document.createTextNode('\n'));
    });
  }

  function highlightProseCode() {
    document.querySelectorAll('.lang-content pre > code, .prose-page pre > code').forEach(highlightCodeBlock);
  }

  /* ── Prose font selector ──────────────────────────────── */
  var PROSE_FONTS = ['courier', 'newsreader', 'spectral', 'eb-garamond', 'playfair', 'cormorant', 'libre-baskerville'];
  var PROSE_FONT_KEY = 'monadb-prose-font';
  var PROSE_FONT_DEFAULT = 'courier';

  function applyProseFont(id) {
    document.documentElement.dataset.proseFont = id;
    try { localStorage.setItem(PROSE_FONT_KEY, id); } catch (e) {}
  }

  function initProseFont() {
    var select = document.querySelector('[data-prose-font-select]');
    if (!select) return;
    var saved;
    try { saved = localStorage.getItem(PROSE_FONT_KEY); } catch (e) {}
    var id = PROSE_FONTS.indexOf(saved) >= 0 ? saved : PROSE_FONT_DEFAULT;
    select.value = id;
    applyProseFont(id);
    select.addEventListener('change', function () {
      if (PROSE_FONTS.indexOf(select.value) >= 0) applyProseFont(select.value);
    });
  }

  function wrapTables() {
    document.querySelectorAll('.lang-content table, .prose-page table').forEach(function (table) {
      if (table.parentElement && table.parentElement.classList.contains('table-wrap')) return;
      var wrap = document.createElement('div');
      wrap.className = 'table-wrap';
      table.parentNode.insertBefore(wrap, table);
      wrap.appendChild(table);
    });
  }

  /* ── Boot ─────────────────────────────────────────────── */
  function init() {
    wrapTables();
    document.querySelectorAll('[data-console]').forEach(initConsole);
    document.querySelectorAll('[data-copy]').forEach(initCopy);
    initCopyMarkdown();
    highlightProseCode();
    initProseFont();
    drawAll();
    // Redraw the wordmark once Playfair has actually loaded.
    if (document.fonts && document.fonts.ready) document.fonts.ready.then(drawAll);
    setTimeout(drawAll, 450);

    var resizeTimer;
    window.addEventListener('resize', function () {
      clearTimeout(resizeTimer);
      resizeTimer = setTimeout(drawAll, 160);
    });
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
}());
