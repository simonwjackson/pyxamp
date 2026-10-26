.pragma library
.import "playlist.js" as Playlist
// The library view as a Finder outline, for the Platinum face. Pure.
//
// The shared model (lib/playlist.js) shows one level of the library path at a
// time; the keys move in that level. The Platinum face draws every open level
// along the path, with triangles and indents, and the deepest level's rows are
// the same rows, in the same order, as the model's: `at` is a row's index in
// model(ctx).rows, so a key or a click on it does exactly what it does on the
// Winamp face. A row of a higher level carries `level` and `idx` instead: a
// click there folds what is below it (Face.qml libFoldTo).
//
// -> [{ depth, level, idx, at, tri: "open" | "closed" | null, row }]
//    row: the model row (l, r, colour, ref, ...); r loses the Winamp face's " >".

function levelCtx(ctx, k) {
    var c = {};
    for (var key in ctx) c[key] = ctx[key];
    c.view = "library"; c.prompt = null;
    c.lib = { path: ctx.lib.path.slice(0, k), sel: 0 };
    return c;
}

function matches(row, e) {
    var r = row.ref;
    if (!r || !e) return false;
    if (r.type === "source") return e.node === undefined && r.source === e.source;
    if (r.type === "group") return r.source === e.source && r.node === e.node;
    if (r.type === "item") return r.item.source === e.source && r.item.id === e.node;
    return false;
}
function openable(row) {
    var r = row.ref;
    if (!r) return false;
    if (r.type === "source") return row.r !== "unreachable" && row.r !== "connecting";
    if (r.type === "group") return true;
    if (r.type === "item") return !!r.item.openable;
    return false;
}
function tidy(row) {
    var out = {};
    for (var k in row) out[k] = row[k];
    if (typeof out.r === "string") out.r = out.r.replace(/\s*>$/, "");
    return out;
}

function rows(ctx) {
    var path = ctx.lib.path, out = [];
    function level(k, depth) {
        var m = Playlist.model(levelCtx(ctx, k)), deepest = k === path.length, found = false;
        for (var i = 0; i < m.rows.length; i++) {
            var row = m.rows[i], open = !deepest && !found && matches(row, path[k]);
            out.push({ depth: depth, level: k, idx: i, at: deepest ? i : -1, tri: open ? "open" : openable(row) ? "closed" : null, row: tidy(row) });
            if (open) { found = true; level(k + 1, depth + 1); }
        }
        // A level opened from elsewhere (the jump box opens an album that is not
        // in the page shown above it): it still gets its open row.
        if (!deepest && !found) {
            out.push({ depth: depth, level: k, idx: -1, at: -1, tri: "open", row: { l: path[k].label, colour: "normal" } });
            level(k + 1, depth + 1);
        }
    }
    level(0, 0);
    return out;
}

// The outline index of the model's selected row (the deepest level's), or -1.
function selIndex(outline, sel) {
    for (var i = 0; i < outline.length; i++) if (outline[i].at === sel && sel >= 0) return i;
    return -1;
}
