.pragma library
.import "lcdtext.js" as Lcd
// The Platinum face's art as whole-sp rectangles, and its small layout rules.
// Pure: a palette (lib/platinum-tokens.js) and a state in, a list of
// { x, y, w, h, c } out, which PlatinumRects.qml draws. Every bevel is one sp,
// so at k physical px per sp it is exactly k px, as on the desktop.
// The primitives, the glyph maps and the geometry are ported from the approved
// mock's Platinum renderer; the glyph maps are its own art.

var MW = 300;

function R(out, x, y, w, h, c) { if (w > 0 && h > 0 && c) out.push({ x: x, y: y, w: w, h: h, c: c }); }
function P(out, x, y, c) { R(out, x, y, 1, 1, c); }
function frame(out, x, y, w, h, tl, br) { R(out, x, y, w, 1, tl); R(out, x, y, 1, h, tl); R(out, x, y + h - 1, w, 1, br); R(out, x + w - 1, y, 1, h, br); }
function bevel(out, x, y, w, h, tl, br) {
    if (tl) { R(out, x, y, w - 1, 1, tl); R(out, x, y, 1, h - 1, tl); }
    if (br) { R(out, x + 1, y + h - 1, w - 1, 1, br); R(out, x + w - 1, y + 1, 1, h - 1, br); }
}
// A sunken well: a fill, then the bevel reversed (flat: a 1-sp outline).
function sunken(out, A, x, y, w, h, fill) {
    if (fill) R(out, x, y, w, h, fill);
    if (A.flat) frame(out, x, y, w, h, A.edge, A.edge); else bevel(out, x, y, w, h, A.lo, A.hi);
}
function ridgeH(out, A, x, y, w) { if (A.flat) R(out, x, y, w, 1, A.ridge); else { R(out, x, y, w, 1, A.lo); R(out, x, y + 1, w, 1, A.hi); } }

// A Platinum push or bevel button: a 1-sp outline with clipped corners and a
// raised bevel; pressed or "on" sinks it. -> the content offset (1 when sunk).
function button(out, A, x, y, w, h, o) {
    o = o || {};
    var down = !!(o.pressed || o.on);
    R(out, x + 1, y, w - 2, 1, A.btnEdge); R(out, x + 1, y + h - 1, w - 2, 1, A.btnEdge); R(out, x, y + 1, 1, h - 2, A.btnEdge); R(out, x + w - 1, y + 1, 1, h - 2, A.btnEdge);
    R(out, x + 1, y + 1, w - 2, h - 2, o.pressed ? A.press : o.on ? A.well : A.raised);
    if (!A.flat) { if (down) bevel(out, x + 1, y + 1, w - 2, h - 2, A.lo, A.hi); else bevel(out, x + 1, y + 1, w - 2, h - 2, A.hi, A.lo); }
    else if (o.on) frame(out, x + 1, y + 1, w - 2, h - 2, A.btnEdge, A.btnEdge);
    return down ? 1 : 0;
}
function mix(a, b, t) {
    function hx(c) { var n = parseInt(c.slice(1, 7), 16); return [n >> 16, (n >> 8) & 255, n & 255]; }
    var A = hx(a), B = hx(b), s = "#";
    for (var i = 0; i < 3; i++) { var v = Math.round(A[i] + (B[i] - A[i]) * t).toString(16); s += v.length < 2 ? "0" + v : v; }
    return s;
}
// An AppleCD-style indicator lamp, 5 x 5.
function lamp(out, A, x, y, on, dim) {
    R(out, x, y, 5, 5, A.flat ? A.btnEdge : A.lo);
    R(out, x + 1, y + 1, 3, 3, dim ? A.lampOff : on ? A.lampOn : A.lampOff);
    if (on && !dim) P(out, x + 1, y + 1, A.lampOnHi);
}
// A small solid triangle in a 9 x 9 box.
function tri(out, x, y, dir, col) {
    var i;
    if (dir === "down") for (i = 0; i < 5; i++) R(out, x + i, y + 2 + i, 9 - 2 * i, 1, col);
    else if (dir === "right") for (i = 0; i < 5; i++) R(out, x + 2 + i, y + i, 1, 9 - 2 * i, col);
    else if (dir === "up") for (i = 0; i < 5; i++) R(out, x + 4 - i, y + 2 + i, 1 + 2 * i, 1, col);
    else if (dir === "left") for (i = 0; i < 5; i++) R(out, x + 2 + i, y + 4 - i, 1, 1 + 2 * i, col);
}
// The Platinum disclosure triangle (11 x 11): outlined and filled.
function discl(out, A, x, y, open, col) {
    var i, fill = A.discl;
    if (open) {
        for (i = 0; i < 6; i++) R(out, x + i, y + 2 + i, 11 - 2 * i, 1, fill);
        R(out, x, y + 2, 11, 1, col);
        for (i = 0; i < 6; i++) { P(out, x + i, y + 2 + i, col); P(out, x + 10 - i, y + 2 + i, col); }
    } else {
        for (i = 0; i < 6; i++) R(out, x + 2 + i, y + i, 1, 11 - 2 * i, fill);
        R(out, x + 2, y, 1, 11, col);
        for (i = 0; i < 6; i++) { P(out, x + 2 + i, y + i, col); P(out, x + 2 + i, y + 10 - i, col); }
    }
}

// Slider thumbs: a bevelled block with a point, down (horizontal sliders) or right (vertical).
function spanShape(out, x0, y0, spans, fill, edge, hiCol, loCol) {
    var r;
    for (r = 0; r < spans.length; r++) {
        var a = spans[r][0], b = spans[r][1];
        R(out, x0 + a, y0 + r, b - a, 1, edge);
        if (r > 0 && r < spans.length - 1 && b - a > 2) R(out, x0 + a + 1, y0 + r, b - a - 2, 1, fill);
    }
    if (hiCol) {
        R(out, x0 + spans[1][0] + 1, y0 + 1, spans[1][1] - spans[1][0] - 2, 1, hiCol);
        for (r = 1; r < spans.length - 1; r++) if (spans[r + 1] && spans[r + 1][0] === spans[r][0]) P(out, x0 + spans[r][0] + 1, y0 + r, hiCol);
    }
    if (loCol) for (r = 2; r < spans.length - 1; r++) if (spans[r][1] - spans[r][0] > 3) P(out, x0 + spans[r][1] - 2, y0 + r, loCol);
}
function thumbH(out, A, x, y, w, h, pressed, dim) {
    var body = h - 5, spans = [];
    for (var r = 0; r < h; r++) { var i = Math.max(0, r - body + 1); spans.push([i, w - i]); }
    var plain = A.flat || pressed;
    spanShape(out, x, y, spans, pressed ? A.press : dim ? A.well : A.raised, A.btnEdge, plain ? null : A.hi, plain ? null : A.lo);
    R(out, x + Math.floor(w / 2), y + 3, 1, body - 3, A.flat ? A.btnEdge : A.lo);
}
function thumbV(out, A, x, y, w, h, pressed, dim) {
    var body = w - 5, f = pressed ? A.press : dim ? A.well : A.raised;
    for (var c = 0; c < w; c++) {
        var i = Math.max(0, c - body + 1), a = i, b = h - i;
        R(out, x + c, y + a, 1, b - a, A.btnEdge);
        if (c > 0 && c < w - 1 && b - a > 2) R(out, x + c, y + a + 1, 1, b - a - 2, f);
    }
    if (!A.flat && !pressed) { R(out, x + 1, y + 1, body - 1, 1, A.hi); R(out, x + 1, y + 1, 1, h - 2, A.hi); R(out, x + 2, y + h - 2, body - 2, 1, A.lo); }
    R(out, x + 3, y + Math.floor(h / 2), body - 3, 1, A.flat ? A.btnEdge : A.lo);
}

// A horizontal Platinum slider at its groove (x, y, w): groove 6 sp, thumb 11 x 14 pointing down.
// o: { frac (null: no thumb), fillTo, fillCol, notch, pressed, dim }
function hslider(out, A, x, y, w, o) {
    sunken(out, A, x, y + 4, w, 6, o.groove || (A.flat ? "#000000" : A.well));
    if (o.fillTo !== null && o.fillTo !== undefined) {
        var fw = Math.round((w - 2) * Math.max(0, Math.min(1, o.fillTo)));
        R(out, x + 1, y + 5, fw, 4, o.fillCol || A.fill); R(out, x + 1, y + 5, fw, 1, A.fillHi);
    }
    if (o.notch !== null && o.notch !== undefined) { var nx = x + 5 + Math.round((w - 11) * o.notch); R(out, nx, y + 12, 1, 3, A.text2); R(out, nx, y, 1, 2, A.text2); }
    if (o.frac !== null && o.frac !== undefined) thumbH(out, A, x + Math.round((w - 11) * Math.max(0, Math.min(1, o.frac))), y, 11, 14, !!o.pressed, !!o.dim);
}

// One EQ slider at x: groove from ty, th tall; db -12..12. o: { off, pressed }
function vslider(out, A, x, ty, th, db, o) {
    sunken(out, A, x + 4, ty, 6, th, o.off ? A.well : (A.flat ? "#000000" : A.well));
    var f = (12 - db) / 24, fy = ty + 1 + Math.round(f * (th - 2)), zy = ty + 1 + Math.round(0.5 * (th - 2));
    if (!o.off) R(out, x + 5, Math.min(fy, zy), 4, Math.abs(fy - zy) + 1, db >= 0 ? A.fill : A.fillHi);
    for (var i = 0; i <= 4; i++) R(out, x + 14, ty + 2 + Math.round(i * (th - 5) / 4), i === 2 ? 4 : 3, 1, A.text2);
    thumbV(out, A, x, fy - 5, 15, 11, !!o.pressed, !!o.off);
}

// ---------- glyphs (the mock's own pixel maps) ----------
var MAPS = {
    prev: ["#.....#...#", "#....##..##", "#...###.###", "#..########", "#.#########", "#..########", "#...###.###", "#....##..##", "#.....#...#"],
    play: ["#......", "##.....", "###....", "####...", "#####..", "####...", "###....", "##.....", "#......"],
    pause: ["###.###", "###.###", "###.###", "###.###", "###.###", "###.###", "###.###", "###.###", "###.###"],
    stop: ["#########", "#########", "#########", "#########", "#########", "#########", "#########", "#########", "#########"],
    next: ["#...#.....#", "##..##....#", "###.###...#", "########..#", "#########.#", "########..#", "###.###...#", "##..##....#", "#...#.....#"],
    eject: ["....#....", "...###...", "..#####..", ".#######.", "#########", ".........", "#########", "#########"],
    back: ["...#...", "..##...", ".######", "#######", ".######", "..##...", "...#..."],
    fwd: ["...#...", "...##..", "######.", "#######", "######.", "...##..", "...#..."],
    speaker: ["...#....", "..##..#.", "####...#", "####...#", "####...#", "..##..#.", "...#...."],
    pausemini: ["##.##", "##.##", "##.##", "##.##", "##.##", "##.##", "##.##"],
    check: ["......#", ".....##", "#...##.", "##.##..", ".###...", "..#...."],
    star: ["....#....", "....#....", "...###...", "..#.#.#..", "####.####", "..#.#.#..", "...###...", "....#....", "....#...."],
    square: ["#######", "#######", "#######", "#######", "#######", "#######", "#######"],
    mstop: ["#####", "#####", "#####", "#####", "#####"],
    mpause: ["##.##", "##.##", "##.##", "##.##", "##.##"],
    mplay: ["#...", "##..", "###.", "##..", "#..."],
    mprev: ["#..#", "#.##", "####", "#.##", "#..#"],
    mnext: ["#..#", "##.#", "####", "##.#", "#..#"],
};
function glyphW(name) { return MAPS[name][0].length; }
function glyphH(name) { return MAPS[name].length; }
// One rectangle per run of "#" in a row.
function glyph(out, name, x, y, col) {
    var rows = MAPS[name];
    for (var j = 0; j < rows.length; j++) {
        var row = rows[j], i = 0;
        while (i < row.length) {
            if (row[i] !== "#") { i++; continue; }
            var s = i; while (i < row.length && row[i] === "#") i++;
            R(out, x + s, y + j, i - s, 1, col);
        }
    }
}

// ---------- the seven-segment clock digit, 9 x 15 sp ----------
var SEGS = {
    a: [[1, 0, 7, 1], [2, 1, 5, 1]], d: [[2, 13, 5, 1], [1, 14, 7, 1]], g: [[2, 6, 5, 1], [2, 7, 5, 1]],
    f: [[0, 1, 1, 6], [1, 2, 1, 4]], b: [[8, 1, 1, 6], [7, 2, 1, 4]], e: [[0, 8, 1, 6], [1, 9, 1, 4]], c: [[8, 8, 1, 6], [7, 9, 1, 4]],
};
var DIGSEG = { 0: "abcdef", 1: "bc", 2: "abged", 3: "abgcd", 4: "fgbc", 5: "afgcd", 6: "afgedc", 7: "abc", 8: "abcdefg", 9: "abcdfg", "-": "g" };
// d: 0..9, "-" or null (ghost only).
function seg7(out, d, x, y, col, ghost) {
    var on = d === null || d === undefined ? "" : DIGSEG[d] || "";
    for (var s in SEGS) for (var k = 0; k < SEGS[s].length; k++) {
        var r = SEGS[s][k];
        R(out, x + r[0], y + r[1], r[2], r[3], on.indexOf(s) >= 0 ? col : ghost);
    }
}

// ---------- window chrome ----------
// An 11 x 11 title-bar box: close (plain), zoom or collapse.
function titleBox(out, A, x, y, kind, pressed) {
    R(out, x, y, 11, 11, A.flat ? A.btnEdge : A.lo); R(out, x + 1, y + 1, 10, 10, A.flat ? A.btnEdge : A.hi);
    R(out, x + 1, y + 1, 9, 9, A.btnEdge); R(out, x + 2, y + 2, 7, 7, pressed ? A.press : A.raised);
    if (!A.flat && !pressed) { R(out, x + 2, y + 2, 7, 1, A.hi); R(out, x + 2, y + 2, 1, 7, A.hi); R(out, x + 3, y + 8, 6, 1, A.lo); R(out, x + 8, y + 3, 1, 6, A.lo); }
    if (kind === "zoom") frame(out, x + 2, y + 2, 5, 5, A.btnEdge, A.btnEdge);
    if (kind === "collapse") { R(out, x + 2, y + 4, 7, 1, A.btnEdge); R(out, x + 2, y + 6, 7, 1, A.btnEdge); }
}
function stripes(out, A, x, y, w) { for (var j = 0; j < 4; j++) { R(out, x, y + j * 3, w, 1, A.stripeHi); if (A.stripeLo) R(out, x, y + j * 3 + 1, w, 1, A.stripeLo); } }

// The 16-sp windoid title bar of a 300-sp window. titleW: the title's width (0: none).
// pressed: "close" | "zoom" | "collapse" | "". zoomOn: double size is on.
function titleBar(A, titleW, pressed, zoomOn) {
    var out = [];
    R(out, 1, 1, MW - 2, 16, A.face);
    if (!A.flat) { R(out, 1, 1, MW - 2, 1, A.hi); R(out, 1, 1, 1, 16, A.hi); R(out, MW - 2, 2, 1, 15, A.lo); }
    R(out, 0, 17, MW, 1, A.edge);
    titleBox(out, A, 6, 3, "close", pressed === "close");
    titleBox(out, A, MW - 31, 3, "zoom", pressed === "zoom" || zoomOn);
    titleBox(out, A, MW - 17, 3, "collapse", pressed === "collapse");
    if (titleW > 0) {
        var tx = Math.round(MW / 2 - titleW / 2);
        stripes(out, A, 21, 4, tx - 6 - 21); stripes(out, A, tx + titleW + 6, 4, MW - 35 - (tx + titleW + 6));
    }
    return out;
}

// A section's 17-sp disclosure header at y (section coordinates).
function header(A, open) {
    var out = [];
    R(out, 1, 0, MW - 2, 17, A.face);
    ridgeH(out, A, 1, 0, MW - 2);
    discl(out, A, 8, 4, open, A.text);
    return out;
}

// The Platinum scroll bar: 15 sp wide, arrow boxes at both ends.
// frac: 0..1 scrolled; size: the visible share (1: no thumb).
function scrollBar(A, x, y, h, frac, size, pressed) {
    var out = [];
    R(out, x, y, 15, h, A.edge); R(out, x + 1, y, 14, h, A.flat ? "#000000" : A.well);
    if (!A.flat) R(out, x + 1, y, 1, h, A.lo);
    function arrow(yy, dir) {
        R(out, x + 1, yy, 14, 14, A.raised);
        if (!A.flat) bevel(out, x + 1, yy, 14, 14, A.hi, A.lo);
        R(out, x + 1, dir === "up" ? yy + 14 : yy - 1, 14, 1, A.edge);
        tri(out, x + 4, yy + 3, dir, A.glyph);
    }
    arrow(y, "up"); arrow(y + h - 14, "down");
    var g = scrollThumb(h, frac, size);
    if (g) {
        var ty = y + g.top;
        R(out, x + 1, ty, 14, g.size, A.edge);
        R(out, x + 2, ty + 1, 12, g.size - 2, pressed ? A.press : A.raised);
        if (!A.flat) bevel(out, x + 2, ty + 1, 12, g.size - 2, A.hi, A.lo);
        for (var i = -2; i <= 2; i += 2) {
            R(out, x + 4, ty + Math.floor(g.size / 2) + i, 8, 1, A.flat ? A.btnEdge : A.lo);
            if (!A.flat) R(out, x + 4, ty + Math.floor(g.size / 2) + i + 1, 8, 1, A.hi);
        }
    }
    return out;
}
// The thumb inside a bar h tall: { top, size, track } (from the bar's top), or null.
function scrollThumb(h, frac, size) {
    var track = h - 30;
    if (size >= 1 || track <= 0) return null;
    var th = Math.max(16, Math.round(track * Math.min(1, size)));
    return { top: 15 + Math.round((track - th) * Math.max(0, Math.min(1, frac || 0))), size: th, track: track };
}
function growBox(A, x, y) {
    var out = [];
    for (var i = 0; i < 3; i++) for (var j = 0; j <= i * 3 + 2; j++) P(out, x + 6 - j, y + 6 - (i * 3 + 2 - j), A.flat ? A.btnEdge : A.lo);
    return out;
}

// ---------- the EQ curve on its small LCD ----------
// -> one offset per column (-5..5 sp from the middle), n columns, points at x0 + i * pitch.
function eqCurve(bands, n, x0, pitch) {
    var pts = [], out = [];
    for (var i = 0; i < 10; i++) pts.push([x0 + i * pitch, bands[i]]);
    for (var x = 0; x < n; x++) {
        var v;
        if (x <= pts[0][0]) v = pts[0][1];
        else if (x >= pts[9][0]) v = pts[9][1];
        else {
            var j = Math.min(8, Math.floor((x - x0) / pitch)), p = (x - pts[j][0]) / pitch;
            var a = pts[Math.max(0, j - 1)][1], b = pts[j][1], c = pts[j + 1][1], d = pts[Math.min(9, j + 2)][1];
            v = b + 0.5 * p * (c - a + p * (2 * a - 5 * b + 4 * c - d + p * (3 * (b - c) + d - a)));
        }
        out.push(Math.max(-5, Math.min(5, Math.round(v / 12 * 5))));
    }
    return out;
}

// ---------- the list view ----------
var SOURCE_NAME = { PYX: "Pyxis", JF: "Jellyfin", OSS: "Ossicle" };
var TAG_SOURCE = { PYX: "pyxis", JF: "jellyfin", OSS: "ossicle" };

// The queue's Finder header line: "14 items, 4:10 of 1:09:31".
function fmt(ms) {
    var s = Math.max(0, Math.floor(ms / 1000)), h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, x = s % 60;
    function p2(n) { return (n < 10 ? "0" : "") + n; }
    return h ? h + ":" + p2(m) + ":" + p2(x) : m + ":" + p2(x);
}
function queueInfo(queue, selOccurrence) {
    var tot = 0, sel = 0;
    for (var i = 0; i < queue.length; i++) {
        var d = queue[i].item.durationMs;
        if (d === undefined) continue;
        tot += d;
        if (queue[i].occurrence === selOccurrence) sel = d;
    }
    var n = queue.length.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
    return n + (queue.length === 1 ? " item, " : " items, ") + fmt(sel) + " of " + fmt(tot);
}

// A queue or album row's number and name: the model writes "12. Name".
function splitNumber(l) {
    var m = /^(\d+)\. (.*)$/.exec(l || "");
    return m ? { n: m[1], name: m[2] } : { n: "", name: l || "" };
}

// Column widths for the list (sp): [[title, width]], width 0 takes the rest.
// numberW and timeW fit the widest number and time of the rows shown.
function columns(kind, numberW, timeW) {
    if (kind === "queue") return [["#", Math.max(30, numberW + 20)], ["Name", 0], ["Time", Math.max(46, timeW + 14)]];
    if (kind === "album") return [["#", Math.max(30, numberW + 20)], ["Name", 0], ["Length", Math.max(46, timeW + 14)]];
    if (kind === "library") return [["Name", 0], ["Items", 70]];
    if (kind === "jump") return [["Name", 0], ["Where", 60], ["Time", Math.max(44, timeW + 12)]];
    return [];
}
function columnX(cols, x0, listW) {
    var fixed = 0, i;
    for (i = 0; i < cols.length; i++) fixed += cols[i][1];
    var out = [], x = x0;
    for (i = 0; i < cols.length; i++) { var w = cols[i][1] || listW - fixed; out.push([x, w]); x += w; }
    return out;
}

// Pop-up buttons of the bottom bar: their x and widths for the label widths
// given (Add, Remove, Select, Misc, Library). Font 11, or 10 when 11 does not fit.
function bottomBar(widths11, widths10) {
    function need(ws) { var s = 0; for (var i = 0; i < 4; i++) s += Math.ceil(ws[i]) + 18 + 3; return s + Math.ceil(ws[4]) + 22; }
    var size = need(widths11) > MW - 7 - 16 ? 10 : 11, ws = size === 11 ? widths11 : widths10, x = 7, out = [];
    for (var i = 0; i < 4; i++) { var w = Math.ceil(ws[i]) + 18; out.push({ x: x, w: w }); x += w + 3; }
    var lw = Math.ceil(ws[4]) + 22;
    out.push({ x: Math.max(x + 3, MW - 16 - lw), w: lw });
    return { size: size, buttons: out };
}

// The LCD's line 2 for a message that wraps, else the format line. lcdW: Lcd.width.
function formatLine(format) {
    if (!format || (!format.left && !format.khz)) return null;
    return { left: format.left ? format.left + " " + format.leftUnit : "", right: format.khz ? format.khz + " kHz" : "" };
}
function lcdWidth(s) { return Lcd.width(s); }
