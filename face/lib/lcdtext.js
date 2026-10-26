.pragma library
.import "lcdfont.js" as Font
// Layout of the Platinum face's LCD type (1-bit ChicagoFLF 12, lib/lcdfont.js).
// Pure: strings in, glyph cells out. One font pixel is one skin pixel.

var ELLIPSIS = "\u2026";

// Characters the font lacks fall back to their base letter, then to "?".
function glyph(cp) {
    var g = Font.glyphs[cp];
    if (g) return g;
    var base = String.fromCharCode(cp).normalize("NFD").charCodeAt(0);
    return Font.glyphs[base] || Font.glyphs[63];
}

function chars(s) { var out = []; for (var ch of String(s || "")) out.push(ch.codePointAt(0)); return out; }

function width(s) {
    var w = 0, cs = chars(s);
    for (var i = 0; i < cs.length; i++) w += glyph(cs[i])[2];
    return w;
}

// -> [{ x, sx, sy }]: draw a Font.cell x Font.line cut of assets/lcd-font.png at x.
function cells(s, ink) {
    var row = Math.max(0, Font.inks.indexOf(ink)), out = [], x = 0, cs = chars(s);
    for (var i = 0; i < cs.length; i++) {
        var g = glyph(cs[i]);
        if (cs[i] !== 32) out.push({ x: x - Font.pad, sx: g[0], sy: (row * Font.lines + g[1]) * Font.line });
        x += g[2];
    }
    return out;
}

// Cut to max sp with an ellipsis.
function fit(s, max) {
    s = String(s || "");
    if (width(s) <= max) return s;
    var cs = Array.from(s), lo = 0, hi = cs.length;
    while (lo < hi) { var m = (lo + hi + 1) >> 1; if (width(cs.slice(0, m).join("") + ELLIPSIS) <= max) lo = m; else hi = m - 1; }
    return cs.slice(0, lo).join("").replace(/\s+$/, "") + ELLIPSIS;
}

// A short message that does not fit wraps onto a second line (the mock's rule):
// -> [line1, line2 or ""]. The second line is cut if it is still too long.
function wrap(s, max) {
    s = String(s || "");
    if (width(s) <= max) return [s, ""];
    var words = s.split(" "), a = "";
    while (words.length && width((a ? a + " " : "") + words[0]) <= max) a += (a ? " " : "") + words.shift();
    if (!a) return [fit(s, max), ""];
    return [a, fit(words.join(" "), max)];
}

// The title's scroll: hold 1.5 s, then 24 sp/s in whole sp, looping with a 40-sp gap.
// -> { offset, loop, span }
var HOLD_MS = 1500, SPEED = 24, GAP = 40;
function scroll(textW, boxW, t0, now, reduced) {
    if (textW <= boxW || reduced) return { offset: 0, loop: false, span: 0 };
    var span = textW + GAP, period = HOLD_MS + span / SPEED * 1000;
    var ph = ((now - t0) % period + period) % period;
    return { offset: ph < HOLD_MS ? 0 : Math.min(span, Math.floor((ph - HOLD_MS) / 1000 * SPEED)), loop: true, span: span };
}
