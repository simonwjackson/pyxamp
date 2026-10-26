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

// The two faces' column geometry (sp). The Winamp face drops a folded section;
// the Platinum face keeps its 17-sp disclosure header, has a 1-sp frame under
// the column, and grows its playlist by two 16-sp Finder rows per step
// (docs/brief.md, Platinum face: main 150, EQ 122, playlist 82 + 16 x (4 + 2n)).
var GEOMETRY = {
    winamp: { w: W, main: MAIN_H, shade: SHADE_H, eq: EQ_H, folded: 0, plMin: PL_MIN, plStep: PL_STEP, frame: 0 },
    platinum: { w: 300, main: 150, shade: 18, eq: 122, folded: 17, plMin: 82 + 16 * 4, plStep: 32, frame: 1 },
};
function geometry(face) { return GEOMETRY[face] || GEOMETRY.winamp; }

function sectionHeights(want, n, face) {
    var g = geometry(face);
    return {
        main: want.shade ? g.shade : g.main,
        eq: want.eq ? g.eq : g.folded,
        pl: want.pl ? g.plMin + g.plStep * n : g.folded,
        frame: g.frame,
    };
}

// mon:  { width, height } in physical px, scale (the compositor's, may be fractional)
// want: { dbl, eq, pl, shade, plN }
// face: "winamp" (default) | "platinum"
// -> { k, plN, refused, fits, spW, spH, physW, physH, logicalW, logicalH, contentScale }
function plan(mon, want, face) {
    var g = geometry(face);
    var scale = mon.scale > 0 ? mon.scale : 1;
    var base = Math.max(1, Math.round(scale)), margin = 2 * MARGIN_LP * scale;
    function spH(n) { var s = sectionHeights(want, n, face); return s.main + s.eq + s.pl + s.frame; }
    function fit(k) {
        var n = want.plN;
        while (n > 0 && k * spH(n) > mon.height - margin) n--;
        return { n: n, ok: k * spH(n) <= mon.height - margin && g.w * k <= mon.width };
    }
    var k = base * (want.dbl ? 2 : 1), f = fit(k), refused = false;
    if (!f.ok && k > base) { k = base; f = fit(k); refused = true; }
    var h = spH(f.n);
    return {
        k: k, plN: f.n, refused: refused, fits: f.ok,
        spW: g.w, spH: h, physW: g.w * k, physH: h * k,
        // The compositor scale is accounted for here, once: the content is drawn
        // at contentScale logical px per skin pixel, which is k physical px.
        contentScale: k / scale,
        logicalW: Math.ceil(g.w * k / scale), logicalH: Math.ceil(h * k / scale),
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
