.pragma library
.import "eq.js" as Eq
// Local, non-sensitive view preferences. Stored as JSON at
// $XDG_STATE_HOME/pyxamp/face.json (Prefs.qml). No credentials live here.
// sanitize() accepts any parsed value and returns a complete, valid record,
// so a damaged or older file never breaks the face.

var VERSION = 1;
var VIS_MODES = ["spec", "osc", "off"];

function defaults() {
    return {
        version: VERSION,
        shown: true,          // SUPER+Z state, so login brings it back as it was left
        onTop: true,          // A: on top (layer top) or on the desktop (layer bottom)
        dbl: false,           // Ctrl+D
        shade: false,         // windowshade
        eqOpen: true,
        plOpen: true,
        plN: 3,               // playlist height steps of 29 sp
        timeMode: "elapsed",
        vis: "spec",
        reducedMotion: false,
        eq: { on: true, auto: false, pre: 0, bands: Eq.preset("flat").bands },
        eqPresets: [],        // the user's own presets: [{ name, pre, bands }] (PRESETS > Save as new preset)
        balance: 0,           // -1 left .. 1 right, Pyxamp's own audio (balance.set)
        albumMode: false,     // the queue as one row per album (G)
        positions: {},        // monitor name -> { x, y } logical px
    };
}

function bool(v, d) { return typeof v === "boolean" ? v : d; }
function dbValue(v) { return typeof v === "number" && isFinite(v) ? Math.max(-12, Math.min(12, Math.round(v))) : 0; }

function sanitize(raw) {
    var d = defaults();
    if (raw === null || typeof raw !== "object" || Array.isArray(raw)) return d;
    var out = defaults();
    var flags = ["shown", "onTop", "dbl", "shade", "eqOpen", "plOpen", "reducedMotion", "albumMode"];
    for (var i = 0; i < flags.length; i++) out[flags[i]] = bool(raw[flags[i]], d[flags[i]]);
    if (typeof raw.plN === "number" && isFinite(raw.plN)) out.plN = Math.max(0, Math.min(6, Math.round(raw.plN)));
    out.timeMode = raw.timeMode === "remaining" ? "remaining" : "elapsed";
    out.vis = VIS_MODES.indexOf(raw.vis) >= 0 ? raw.vis : d.vis;
    var eq = raw.eq;
    if (eq && typeof eq === "object") {
        out.eq.on = bool(eq.on, true);
        out.eq.auto = bool(eq.auto, false);
        out.eq.pre = dbValue(eq.pre);
        if (Array.isArray(eq.bands) && eq.bands.length === 10) out.eq.bands = eq.bands.map(dbValue);
    }
    if (typeof raw.balance === "number" && isFinite(raw.balance)) out.balance = Math.max(-1, Math.min(1, Math.round(raw.balance * 100) / 100));
    if (Array.isArray(raw.eqPresets)) {
        for (var j = 0; j < raw.eqPresets.length && out.eqPresets.length < Eq.USER_PRESETS_MAX; j++) {
            var u = raw.eqPresets[j];
            if (u && typeof u.name === "string" && u.name.trim() && Array.isArray(u.bands) && u.bands.length === 10)
                out.eqPresets.push({ name: u.name.trim().slice(0, Eq.NAME_MAX), pre: dbValue(u.pre), bands: u.bands.map(dbValue) });
        }
    }
    var pos = raw.positions;
    if (pos && typeof pos === "object" && !Array.isArray(pos)) {
        for (var name in pos) {
            var p = pos[name];
            if (p && typeof p.x === "number" && typeof p.y === "number" && isFinite(p.x) && isFinite(p.y))
                out.positions[name] = { x: Math.round(p.x), y: Math.round(p.y) };
        }
    }
    return out;
}

// Immutable update: with(prefs, { dbl: true }) -> a new record.
function withChanges(prefs, changes) {
    var out = JSON.parse(JSON.stringify(prefs));
    for (var k in changes) out[k] = changes[k];
    return sanitize(out);
}
