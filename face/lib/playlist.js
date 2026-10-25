.pragma library
.import "view.js" as View
// The playlist section's model: the queue (track or album mode), the library
// view, the jump box, item info and the prompt line, as rows to draw. Pure.
//
// ctx = {
//   snap:   decoded snapshot (protocol.js) or null,  main: View.main(...),
//   view:   "queue" | "library" | "jump" | "info",
//   albumMode, albumOpen (first occurrence of the opened album, or ""), albumSel (row index),
//   sel:    the selected queue occurrence, or "",
//   lib:    { path: [{ source, node?, label, kind? }], sel },  pages: key -> { status, items, nextCursor?, total?, message? },
//   jump:   { text, sel, status: "idle" | "searching" | "ready" | "error", items, message },
//   prompt: { kind, title, label, text, right, hint } | null,
//   conflictPick, info: View.itemInfo(...), caret: "_" or " ",
// }
// A row: { l, alt?, r?, colour, rcolour?, bg?, center?, key?, tag?, tagErr?, tick?, play?,
//          chips?, pick?, divTop?, video?, group?, ref? }. ref says what Enter, a click
// or a right click acts on; rows without ref cannot be selected.

var fmt = View.formatSeconds;
var TAG = { pyxis: "PYX", ossicle: "OSS", jellyfin: "JF" };
var ORDER = { pyxis: 0, ossicle: 1, jellyfin: 2 };
// Thousands with commas ("1,288"); QML's toLocaleString ignores the locale argument.
var GROUPS = /\B(?=(\d{3})+(?!\d))/g;
var HEAD_BG = "#0b3a0b", PROMPT_BG = "#10204a", FOOT_BG = "#0a0a14";

function ms(v) { return fmt(v / 1000); }
function sourceLabel(s) { return View.sourceLabel(s); }
function activeSource(ctx) { return ctx.snap && ctx.snap.own ? ctx.snap.own.source : ""; }
function summaries(ctx) { return ctx.snap && ctx.snap.library._tag === "sources" ? ctx.snap.library.sources : []; }
function summary(ctx, source) { var s = summaries(ctx); for (var i = 0; i < s.length; i++) if (s[i].source === source) return s[i]; return null; }
function pageKey(source, node) { return source + "\u0000" + (node || ""); }

// ---------- titles ----------
var QUEUE_TITLE = { pyxis: "PYXIS QUEUE", jellyfin: "JELLYFIN QUEUE", ossicle: "OSSICLE UP NEXT" };
var ALBUM_TITLE = { pyxis: "PYXIS QUEUE BY ALBUM", jellyfin: "JELLYFIN QUEUE BY ALBUM", ossicle: "OSSICLE UP NEXT BY SHOW" };
var TITLES = ["QUEUE", "PYXIS QUEUE", "JELLYFIN QUEUE", "OSSICLE UP NEXT", "PYXIS QUEUE BY ALBUM", "JELLYFIN QUEUE BY ALBUM",
              "OSSICLE UP NEXT BY SHOW", "LIBRARY", "JUMP TO ITEM", "ITEM INFO", "SAVE AS PLAYLIST", "SAVE EQ PRESET"];
function title(ctx) {
    if (ctx.prompt) return ctx.prompt.title;
    if (ctx.view === "jump") return "JUMP TO ITEM";
    if (ctx.view === "info") return "ITEM INFO";
    if (ctx.view === "library") return "LIBRARY";
    var s = activeSource(ctx);
    if (!s) return "QUEUE";
    return (ctx.albumMode ? ALBUM_TITLE : QUEUE_TITLE)[s];
}
function titleSprite(t) { return "pl-title-" + t.toLowerCase().replace(/ /g, "-"); }

// ---------- queue ----------
function isCurrent(ctx, q) {
    var o = ctx.snap.own;
    return q.status === "current" || (!!o && o.occurrence !== undefined && o.occurrence === q.occurrence);
}
function conflictFor(ctx, it) {
    var c = ctx.snap.conflicts;
    for (var i = 0; i < c.length; i++) if (c[i].item.source === it.source && c[i].item.id === it.id) return c[i];
    return null;
}
function itemName(it, withSubtitle) {
    if (it.kind === "book") return it.title + (it.subtitle ? " - " + it.subtitle : "");
    return (withSubtitle && it.subtitle ? it.subtitle + " - " : "") + it.title;
}
function timeLeft(it) {
    var p = it.progress;
    return p && !p.played && p.positionMs !== undefined && it.durationMs !== undefined ? "-" + ms(Math.max(0, it.durationMs - p.positionMs)) : "";
}
function chipsFor(c) { return c.positions.map(function (p) { return p.device + " " + ms(p.positionMs); }); }

function queueRow(ctx, q, i, number) {
    var it = q.item, cur = isCurrent(ctx, q), following = ctx.main.mode === "following";
    var row = { l: (number || i + 1) + ". " + itemName(it, true), r: it.durationMs !== undefined ? ms(it.durationMs) : "", colour: cur ? "current" : "normal",
                ref: { type: "queue", occurrence: q.occurrence, item: it, index: i } };
    if (cur && following) row.r = "paused";
    else if (it.progress && it.progress.played) { row.r = "played"; row.colour = "dim"; row.tick = true; }
    else if (!cur && timeLeft(it)) row.r = timeLeft(it);
    if (q.status === "failed") { row.colour = "err"; row.r = "cannot play"; row.rcolour = "err"; row.tick = false; }
    else if (q.status === "retrying") { row.r = "retrying"; row.rcolour = "err"; }
    var c = conflictFor(ctx, it);
    if (c) { row.colour = "current"; row.bg = "#2e2208"; row.r = ""; row.chips = chipsFor(c); row.pick = ctx.conflictPick || 0; row.ref.conflict = c; }
    return row;
}

// Album mode: consecutive occurrences with one albumKey become one row; a book stays one row.
function groups(queue) {
    var out = [];
    for (var i = 0; i < queue.length; i++) {
        var q = queue[i], key = q.item.kind === "book" || q.albumKey === undefined ? null : q.albumKey, last = out[out.length - 1];
        if (last && key !== null && last.key === key) last.items.push(i);
        else out.push({ key: key, items: [i], first: q.occurrence });
    }
    return out;
}
function sum(items) {
    var t = 0;
    for (var i = 0; i < items.length; i++) { if (items[i].durationMs === undefined) return -1; t += items[i].durationMs; }
    return t;
}

function albumRows(ctx) {
    var queue = ctx.snap.queue, gs = groups(queue), rows = [];
    for (var g = 0; g < gs.length; g++) {
        var its = gs[g].items.map(function (i) { return queue[i].item; }), f = its[0], n = its.length, num = (g + 1) + ". ";
        var playing = gs[g].items.some(function (i) { return isCurrent(ctx, queue[i]); }), c = playing ? "current" : "normal", tot = sum(its);
        var ref = { type: "album", group: g, first: gs[g].first, items: gs[g].items };
        if (f.kind === "book") {
            var left = timeLeft(f);
            rows.push({ l: num + itemName(f, true), r: left ? "book  " + left : f.durationMs !== undefined ? "book  " + ms(f.durationMs) : "book", colour: c, play: playing, ref: ref });
        } else if (n === 1) {
            var row = queueRow(ctx, queue[gs[g].items[0]], gs[g].items[0], g + 1);
            row.play = playing; row.ref = ref; rows.push(row);
        } else if (f.source === "ossicle") {
            var pl = its.filter(function (x) { return x.progress && x.progress.played; }).length;
            rows.push({ l: num + (f.subtitle || f.title), r: n + " episodes" + (pl ? ", " + pl + " played" : "") + (tot >= 0 ? "  " + ms(tot) : ""), colour: c, play: playing, ref: ref });
        } else {
            var name = (f.subtitle ? f.subtitle + " - " : "") + (f.album || f.title);
            var k = 0; for (var j = 0; j < n; j++) if (isCurrent(ctx, queue[gs[g].items[j]])) k = j + 1;
            rows.push({ l: num + name + (f.year ? " (" + f.year + ")" : ""), alt: num + name, r: (playing ? "now " + k + "/" + n : n + " tracks") + (tot >= 0 ? "  " + ms(tot) : ""), colour: c, play: playing, ref: ref });
        }
    }
    return rows;
}

function openAlbum(ctx) {
    var queue = ctx.snap.queue, gs = groups(queue);
    for (var g = 0; g < gs.length; g++) if (gs[g].first === ctx.albumOpen) {
        var f = queue[gs[g].items[0]].item, n = gs[g].items.length, sw = f.source === "ossicle";
        var name = sw ? (f.subtitle || f.title) : (f.subtitle ? f.subtitle + " - " : "") + (f.album || f.title) + (f.year ? " (" + f.year + ")" : "");
        return {
            head: { l: "< " + name, colour: "normal", bg: HEAD_BG, r: n + (sw ? (n === 1 ? " episode" : " episodes") : (n === 1 ? " track" : " tracks")), ref: { type: "albumBack" } },
            rows: gs[g].items.map(function (i) { return queueRow(ctx, queue[i], i); }),
        };
    }
    return null;
}

function emptyQueue(ctx) {
    var s = activeSource(ctx);
    if (s) return [{ l: "" }, { l: "Nothing queued in " + sourceLabel(s) + ".", colour: "dim", center: true }, { l: "Press J to find something,", colour: "dim", center: true }, { l: "or L to open the library.", colour: "dim", center: true }];
    if (summaries(ctx).length) return [{ l: "" }, { l: "Nothing plays.", colour: "dim", center: true }, { l: "Press J to find something,", colour: "dim", center: true }, { l: "or L to open the library.", colour: "dim", center: true }];
    return [{ l: "" }, { l: "No sources yet, so nothing is queued.", colour: "dim", center: true }, { l: "Pyxamp follows the player that", colour: "dim", center: true }, { l: "started playing last.", colour: "dim", center: true }];
}

function queueModel(ctx) {
    var q = ctx.snap ? ctx.snap.queue : [];
    if (!q.length) {
        var orphan = ctx.snap ? ctx.snap.conflicts : [];
        if (!orphan.length) return { rows: emptyQueue(ctx), sel: -1 };
    }
    var foot = ctx.snap.conflicts.length ? { l: "Left, Right: pick a position   Enter: keep it", colour: "dim", bg: FOOT_BG } : null;
    if (ctx.albumMode) {
        var open = ctx.albumOpen ? openAlbum(ctx) : null;
        if (open) return { head: open.head, rows: open.rows, foot: foot, sel: selIndex(open.rows, ctx.sel) };
        var rows = albumRows(ctx);
        return { rows: rows, foot: foot, sel: Math.max(0, Math.min(rows.length - 1, ctx.albumSel || 0)) };
    }
    var out = q.map(function (x, i) { return queueRow(ctx, x, i); });
    // A conflict for an item that is not queued still gets its row, first.
    ctx.snap.conflicts.forEach(function (c) {
        if (!q.some(function (x) { return x.item.source === c.item.source && x.item.id === c.item.id; }))
            out.unshift({ l: itemName(c.item, true), colour: "current", bg: "#2e2208", chips: chipsFor(c), pick: ctx.conflictPick || 0, ref: { type: "conflict", conflict: c, item: c.item } });
    });
    return { rows: out, foot: foot, sel: selIndex(out, ctx.sel) };
}
function selIndex(rows, occurrence) {
    if (!occurrence) return -1;
    for (var i = 0; i < rows.length; i++) if (rows[i].ref && rows[i].ref.occurrence === occurrence) return i;
    return -1;
}

// ---------- library ----------
var ALBUMISH = { albums: true, artists: true };
function itemRow(it, i, parentKind) {
    var l, alt, r = "", row;
    switch (it.kind) {
    case "track": l = (i + 1) + ". " + (parentKind === "album" ? it.title : itemName(it, true)); break;
    case "album": l = itemName(it, true) + (it.year ? " (" + it.year + ")" : ""); alt = itemName(it, true); break;
    case "episode": l = parentKind === "show" ? it.title : itemName(it, true); break;
    default: l = itemName(it, false);
    }
    row = { l: l, colour: "normal", ref: { type: "item", item: it } };
    if (alt) row.alt = alt;
    if (it.progress && it.progress.played) { r = "played"; row.colour = "dim"; row.tick = true; }
    else if (timeLeft(it)) r = timeLeft(it);
    else if (it.openable) r = it.kind === "playlist" && it.count !== undefined ? it.count.toString().replace(GROUPS, ",") + " tracks >" : it.count !== undefined ? it.count.toString().replace(GROUPS, ",") + " >" : ">";
    else if (it.durationMs !== undefined) r = ms(it.durationMs);
    else if (it.kind === "station") r = "station";
    if (it.kind === "video") row.video = true;
    row.r = r;
    return row;
}

function crumb(path) {
    var names = ["Library"];
    for (var i = 0; i < path.length; i++) names.push(path[i].label);
    return (path.length ? "< " : "") + names.join(" > ");
}

function libraryModel(ctx) {
    var path = ctx.lib.path, head = { l: crumb(path), colour: "normal", bg: HEAD_BG, ref: path.length ? { type: "libBack" } : { type: "viewBack" } }, rows = [];
    if (!path.length) {
        var all = summaries(ctx), active = activeSource(ctx);
        if (!all.length) rows = [{ l: "" }, { l: "No sources are set up.", colour: "dim", center: true }, { l: "Pyxamp reads them from sources.toml.", colour: "dim", center: true }];
        for (var i = 0; i < all.length; i++) {
            var s = all[i], st = s.status._tag;
            if (st === "unavailable") rows.push({ l: s.label, r: "unreachable", colour: "dim", rcolour: "err", tip: s.label + ": " + s.status.message, ref: { type: "source", source: s.source, label: s.label } });
            else if (st === "connecting") rows.push({ l: s.label, r: "connecting", colour: "dim", ref: { type: "source", source: s.source, label: s.label } });
            else rows.push({ l: s.label, r: s.groups.length + (s.groups.length === 1 ? " group >" : " groups >"), colour: s.source === active ? "current" : "normal", ref: { type: "source", source: s.source, label: s.label } });
        }
    } else if (path.length === 1) {
        var sm = summary(ctx, path[0].source);
        if (!sm) rows = [{ l: path[0].label + " is not set up.", colour: "dim" }];
        else if (sm.status._tag === "unavailable") rows = [{ l: sm.label + " is unreachable: " + sm.status.message, colour: "err" }];
        else if (sm.status._tag === "connecting") rows = [{ l: "Connecting to " + sm.label + "...", colour: "dim" }];
        else if (!sm.groups.length) rows = [{ l: sm.label + " reports no groups.", colour: "dim" }];
        else rows = sm.groups.map(function (g, j) {
            var prev = sm.groups[j - 1];
            return { l: g.label, r: (g.count !== undefined ? g.count.toString().replace(GROUPS, ",") + " " : "") + ">", colour: "normal",
                     divTop: !!prev && !!ALBUMISH[prev.kind] && !ALBUMISH[g.kind], ref: { type: "group", source: sm.source, node: g.id, label: g.label, kind: g.kind } };
        });
    } else {
        var last = path[path.length - 1], pg = ctx.pages[pageKey(last.source, last.node)];
        if (!pg || pg.status === "loading") rows = [{ l: "Loading...", colour: "dim" }];
        else if (pg.status === "error") rows = [{ l: sourceLabel(last.source) + " did not answer: " + pg.message, colour: "err" }];
        else {
            rows = pg.items.map(function (it, k) { return itemRow(it, k, last.kind); });
            if (!rows.length) rows = [{ l: "Nothing here.", colour: "dim" }];
            if (pg.nextCursor !== undefined) rows.push({ l: pg.loadingMore ? "Loading more..." : "More...", r: pg.total !== undefined ? pg.items.length.toString().replace(GROUPS, ",") + " of " + pg.total.toString().replace(GROUPS, ",") : "", colour: "dim", ref: { type: "more" } });
            if (pg.total !== undefined) head.r = pg.total.toString().replace(GROUPS, ",");
        }
    }
    var sel = Math.max(0, Math.min(rows.length - 1, ctx.lib.sel || 0));
    if (!rows[sel] || !rows[sel].ref) sel = firstSelectable(rows);
    return { head: head, rows: rows, sel: sel };
}
function firstSelectable(rows) { for (var i = 0; i < rows.length; i++) if (rows[i].ref) return i; return -1; }

// ---------- jump box ----------
function jumpModel(ctx) {
    var j = ctx.jump, rows = [], active = activeSource(ctx), text = j.text;
    var down = summaries(ctx).filter(function (s) { return s.status._tag === "unavailable"; });
    var items = (j.items || []).slice().sort(function (a, b) { return (ORDER[a.source] || 0) - (ORDER[b.source] || 0); });
    var sources = {};
    for (var i = 0; i < items.length; i++) {
        var it = items[i]; sources[it.source] = true;
        var r = it.durationMs !== undefined ? ms(it.durationMs) : it.kind === "track" || it.kind === "episode" ? "" : it.kind;
        rows.push({ l: it.kind === "album" ? itemName(it, true) + (it.year ? " (" + it.year + ")" : "") : itemName(it, true), alt: itemName(it, true), r: r, colour: "normal", tag: TAG[it.source], video: it.kind === "video", ref: { type: "item", item: it } });
    }
    if (text) for (var d = 0; d < down.length; d++)
        rows.push({ l: down[d].label + " is unreachable; no results from it", colour: "dim", tag: TAG[down[d].source], tagErr: true, group: true, tip: down[d].status.message });
    if (!text) rows.push({ l: "Type to search " + (summaries(ctx).length ? summaries(ctx).map(function (s) { return s.label; }).join(", ").replace(/, ([^,]*)$/, " and $1") : "your sources") + ".", colour: "dim", group: true });
    else if (j.status === "searching" && !items.length) rows.unshift({ l: "Searching...", colour: "dim", group: true });
    else if (j.status === "error") rows.unshift({ l: "Search failed: " + j.message, colour: "err", group: true });
    else if (!items.length && !summaries(ctx).length) rows.unshift({ l: "Nothing found for \"" + text + "\": no sources yet.", colour: "dim", group: true });
    else if (!items.length) rows.unshift({ l: "Nothing found for \"" + text + "\".", colour: "dim", group: true });
    var n = Object.keys(sources).length;
    var right = !text ? (summaries(ctx).length ? "all sources" : "no sources") : j.status === "searching" ? "searching" : items.length ? items.length + " in " + n + " source" + (n === 1 ? "" : "s") : "0 found";
    var head = { l: "Jump: " + text + ctx.caret, fitLeft: "Jump: ", r: right, colour: "current", bg: PROMPT_BG };
    var sel = Math.max(0, Math.min(rows.length - 1, j.sel || 0));
    if (!rows[sel] || !rows[sel].ref) sel = firstSelectable(rows);
    var hint = "Enter: play   Ins: queue   Esc: close", s = sel >= 0 ? rows[sel].ref.item : null;
    if (!items.length) hint = "Esc: close";
    else if (s && s.kind === "video") hint = "Enter: open in mpv, outside Pyxamp";
    else if (s && active && s.source !== active) hint = "Enter: play from " + sourceLabel(s.source) + "; " + sourceLabel(active) + " pauses";
    else if (s && !s.playable && s.openable) hint = "Enter: open in the library   Esc: close";
    return { head: head, rows: rows, sel: sel, foot: { l: hint, colour: "dim", bg: FOOT_BG } };
}

function infoModel(ctx) {
    var i = ctx.info;
    return {
        head: { l: "Item info", r: "Esc: close", colour: "normal", bg: HEAD_BG, ref: { type: "viewBack" } },
        rows: i.rows.length ? i.rows.map(function (kv) { return { key: kv[0], l: kv[1], colour: "normal" }; }) : [{ l: i.empty, colour: "dim" }],
        sel: -1, noSel: true,
    };
}

function base(ctx) {
    switch (ctx.view) {
    case "library": return libraryModel(ctx);
    case "jump": return jumpModel(ctx);
    case "info": return infoModel(ctx);
    }
    return queueModel(ctx);
}

// The prompt line sits over the section like the jump box; the list below stays, inert.
function model(ctx) {
    var m = base(ctx);
    m.title = title(ctx);
    if (ctx.prompt) {
        var P = ctx.prompt;
        m.head = { l: P.label + ": " + P.text + ctx.caret, fitLeft: P.label + ": ", r: P.right || "", colour: "current", bg: PROMPT_BG };
        m.foot = { l: P.hint || "Enter: save   Esc: cancel", colour: "dim", bg: FOOT_BG };
        m.sel = -1; m.noSel = true; m.inert = true;
    }
    return m;
}

// The queue's bottom-bar readout: selected length / total length.
function totals(ctx) {
    var q = ctx.snap ? ctx.snap.queue : [], tot = 0, sel = 0;
    for (var i = 0; i < q.length; i++) {
        var d = q[i].item.durationMs;
        if (d === undefined) continue;
        tot += d;
        if (q[i].occurrence === ctx.sel) sel = d;
    }
    return ms(sel) + "/" + ms(tot);
}
