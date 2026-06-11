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
