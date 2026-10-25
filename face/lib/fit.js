.pragma library
// Size and place the Pyxamp column on one monitor. Pure; ported from the
// approved mock's plan() (docs/mock/pyxamp.js, decision 22).
//
// Scale is the monitor's scale rounded to a whole number; Ctrl+D doubles it.
// If the column does not fit with MARGIN_LP logical px free above and below,
// the playlist first shrinks in 29-sp steps; if it still does not fit, Ctrl+D
// is refused on that monitor. The margin is a guess until measured on Myoko.

var W = 275, MAIN_H = 116, EQ_H = 116, SHADE_H = 14, PL_MIN = 116, PL_STEP = 29;
var MARGIN_LP = 16;

function sectionHeights(want, n) {
    return {
        main: want.shade ? SHADE_H : MAIN_H,
        eq: want.eq ? EQ_H : 0,
        pl: want.pl ? PL_MIN + PL_STEP * n : 0,
    };
}

// mon:  { width, height } in physical px, scale (the compositor's, may be fractional)
// want: { dbl, eq, pl, shade, plN }
// -> { k, plN, refused, fits, spW, spH, physW, physH, logicalW, logicalH, contentScale }
function plan(mon, want) {
    var scale = mon.scale > 0 ? mon.scale : 1;
    var base = Math.max(1, Math.round(scale)), margin = 2 * MARGIN_LP * scale;
    function spH(n) { var s = sectionHeights(want, n); return s.main + s.eq + s.pl; }
    function fit(k) {
        var n = want.plN;
        while (n > 0 && k * spH(n) > mon.height - margin) n--;
        return { n: n, ok: k * spH(n) <= mon.height - margin && W * k <= mon.width };
    }
    var k = base * (want.dbl ? 2 : 1), f = fit(k), refused = false;
    if (!f.ok && k > base) { k = base; f = fit(k); refused = true; }
    var h = spH(f.n);
    return {
        k: k, plN: f.n, refused: refused, fits: f.ok,
        spW: W, spH: h, physW: W * k, physH: h * k,
        // The compositor scale is accounted for here, once: the content is drawn
        // at contentScale logical px per skin pixel, which is k physical px.
        contentScale: k / scale,
        logicalW: Math.ceil(W * k / scale), logicalH: Math.ceil(h * k / scale),
    };
}

// Keep a remembered position reachable on this monitor (logical px).
// pos: { x, y } | undefined; size: { w, h }; mon: { width, height } logical.
function place(pos, size, mon) {
    var x = pos && typeof pos.x === "number" ? pos.x : MARGIN_LP;
    var y = pos && typeof pos.y === "number" ? pos.y : MARGIN_LP;
    x = Math.max(0, Math.min(Math.round(x), Math.max(0, mon.width - size.w)));
    y = Math.max(0, Math.min(Math.round(y), Math.max(0, mon.height - size.h)));
    return { x: x, y: y };
}
