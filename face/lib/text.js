.pragma library
.import "sprites.js" as Sprites
// Layout of the two bitmap fonts. Pure: strings in, glyph cells out.

// Pyx 5x6: fixed 5-sp advance. -> [{ x, sx, sy }] with sx/sy on the sheet.
function text5Cells(s, colour) {
    var t = Sprites.text5, row = Math.max(0, t.colours.indexOf(colour)), out = [];
    for (var i = 0; i < s.length; i++) {
        var c = t.chars.indexOf(s[i]);
        if (c < 0) c = t.chars.indexOf("?");
        if (s[i] === " ") continue;
        out.push({ x: i * 5, sx: c * t.cell, sy: t.top + row * t.height });
    }
    return out;
}

function sysGlyph(cp) {
    var g = Sprites.sys.glyphs[cp];
    return g || Sprites.sys.glyphs[63]; // "?"
}

function sysWidth(s) {
    var w = 0;
    for (var i = 0; i < s.length; i++) w += sysGlyph(s.charCodeAt(i))[3];
    return w;
}

// W95FA Grid: proportional advance. -> [{ x, w, sx, sy }]
function sysCells(s, colour) {
    var f = Sprites.sys, ci = Math.max(0, f.colours.indexOf(colour)), out = [], x = 0;
    for (var i = 0; i < s.length; i++) {
        var g = sysGlyph(s.charCodeAt(i));
        if (s[i] !== " ") out.push({ x: x, w: g[2], sx: g[0], sy: f.top + (ci * f.lines + g[1]) * f.line });
        x += g[3];
    }
    return out;
}

// Cut a string to max sp, ending in "...".
function sysFit(s, max) {
    if (sysWidth(s) <= max) return s;
    var lo = 0, hi = s.length;
    while (lo < hi) { var m = (lo + hi + 1) >> 1; if (sysWidth(s.slice(0, m) + "...") <= max) lo = m; else hi = m - 1; }
    return s.slice(0, lo).replace(/\s+$/, "") + "...";
}

// Keep the end of a typed line in view: "Jump: ...end_".
function sysFitLeft(prefix, s, max) {
    if (sysWidth(prefix + s) <= max) return prefix + s;
    var k = 0;
    while (k < s.length && sysWidth(prefix + "..." + s.slice(k)) > max) k++;
    return prefix + "..." + s.slice(k);
}
