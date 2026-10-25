.pragma library
// Pyxamp control socket, version 1: the face's side of the wire.
// The daemon's Rust types (contracts/pyxamp.schema.json) are canonical. This
// module builds client messages in exactly those shapes and turns each server
// line into a tagged value, so nothing below the link reads raw JSON.

var VERSION = 1;
var MAX_LINE = 1048576; // 1 MiB, the same bound the daemon keeps

var TRANSPORTS = ["playing", "paused", "stopped", "unknown"];
var REPEAT_MODES = ["off", "all", "one"];
var VIS_STATUSES = ["ready", "idle", "unavailable"];
var CAPABILITIES = ["play", "pause", "stop", "previous", "next", "seek", "volume", "repeat", "shuffle"];

function hello() {
    return JSON.stringify({ _tag: "hello", payload: { version: VERSION } });
}

// action: { _tag: "transport.play", payload: {} } and the other step-1 variants.
function command(id, target, action) {
    return JSON.stringify({ _tag: "command", payload: { id: id, target: { busName: target.busName, owner: target.owner }, action: action } });
}

function action(tag, payload) {
    return { _tag: tag, payload: payload || {} };
}

// ---------- decoding: line -> { ok: true, msg } | { ok: false, reason } ----------

function isObject(v) { return v !== null && typeof v === "object" && !Array.isArray(v); }
function isString(v) { return typeof v === "string"; }
function isUInt(v) { return typeof v === "number" && isFinite(v) && v >= 0 && Math.floor(v) === v; }
function isNumber(v) { return typeof v === "number" && isFinite(v); }
function oneOf(v, list) { return list.indexOf(v) >= 0; }

function fail(reason) { return { ok: false, reason: reason }; }

function decodeTarget(t) {
    if (!isObject(t) || !isString(t.busName) || !isString(t.owner)) return null;
    return { busName: t.busName, owner: t.owner };
}

function decodeTrack(t) {
    if (!isObject(t) || !isString(t.title) || !isString(t.artist)) return null;
    var out = { title: t.title, artist: t.artist };
    var strings = ["id", "album", "artworkUrl"];
    for (var i = 0; i < strings.length; i++) {
        var k = strings[i];
        if (t[k] === undefined) continue;
        if (!isString(t[k])) return null;
        out[k] = t[k];
    }
    if (t.durationMs !== undefined) { if (!isUInt(t.durationMs)) return null; out.durationMs = t.durationMs; }
    var numbers = ["bitrateKbps", "sampleRateHz", "bitDepth", "channels"];
    for (var j = 0; j < numbers.length; j++) {
        var n = numbers[j];
        if (t[n] === undefined) continue;
        if (!isNumber(t[n]) || t[n] < 0) return null;
        out[n] = t[n];
    }
    return out;
}

function decodeCapabilities(c) {
    if (!isObject(c)) return null;
    var out = {};
    for (var i = 0; i < CAPABILITIES.length; i++) {
        var k = CAPABILITIES[i];
        if (typeof c[k] !== "boolean") return null;
        out[k] = c[k];
    }
    return out;
}

function decodePlayer(p) {
    if (!isObject(p) || !isObject(p.payload)) return null;
    var q = p.payload;
    if (p._tag === "idle") return { _tag: "idle" };
    if (p._tag === "unavailable") {
        if (!isString(q.code) || !isString(q.message)) return null;
        return { _tag: "unavailable", code: q.code, message: q.message };
    }
    if (p._tag !== "following") return null;
    var target = decodeTarget(q.target), track = decodeTrack(q.track), caps = decodeCapabilities(q.capabilities);
    if (!target || !track || !caps || !isString(q.identity) || !oneOf(q.transport, TRANSPORTS)) return null;
    var out = { _tag: "following", target: target, identity: q.identity, track: track, transport: q.transport, capabilities: caps };
    if (q.positionMs !== undefined) { if (!isUInt(q.positionMs)) return null; out.positionMs = q.positionMs; }
    if (q.volume !== undefined) { if (!isNumber(q.volume)) return null; out.volume = Math.max(0, Math.min(1, q.volume)); }
    if (q.repeat !== undefined) { if (!oneOf(q.repeat, REPEAT_MODES)) return null; out.repeat = q.repeat; }
    if (q.shuffle !== undefined) { if (typeof q.shuffle !== "boolean") return null; out.shuffle = q.shuffle; }
    if (q.rate !== undefined) { if (!isNumber(q.rate) || q.rate < 0) return null; out.rate = q.rate; }
    return out;
}

function decodeSamples(a) {
    if (!Array.isArray(a) || a.length !== 75) return null;
    for (var i = 0; i < 75; i++) if (!isUInt(a[i]) || a[i] > 255) return null;
    return a;
}

function decodeServer(line) {
    if (line.length > MAX_LINE) return fail("line longer than 1 MiB");
    var m;
    try { m = JSON.parse(line); } catch (e) { return fail("not JSON"); }
    if (!isObject(m) || !isString(m._tag) || !isObject(m.payload)) return fail("no _tag or payload");
    var p = m.payload;
    switch (m._tag) {
    case "ready":
        if (p.version !== VERSION || !isString(p.instanceId)) return fail("bad ready");
        return { ok: true, msg: { _tag: "ready", instanceId: p.instanceId } };
    case "state": {
        if (!isUInt(p.revision)) return fail("bad state revision");
        var player = decodePlayer(p.player);
        if (!player) return fail("bad player state");
        if (!isObject(p.library) || p.library._tag !== "noSources") return fail("bad library");
        if (!Array.isArray(p.queue) || p.queue.length !== 0) return fail("bad queue");
        return { ok: true, msg: { _tag: "state", revision: p.revision, player: player, library: { _tag: "noSources" }, queue: [] } };
    }
    case "visualizer": {
        var target = null;
        if (p.target !== undefined) { target = decodeTarget(p.target); if (!target) return fail("bad visualizer target"); }
        var cols = decodeSamples(p.columns), wave = decodeSamples(p.waveform);
        if (!isUInt(p.sequence) || !oneOf(p.status, VIS_STATUSES) || !cols || !wave) return fail("bad visualizer frame");
        if (p.reason !== undefined && !isString(p.reason)) return fail("bad visualizer reason");
        return { ok: true, msg: { _tag: "visualizer", target: target, sequence: p.sequence, status: p.status, columns: cols, waveform: wave, reason: p.reason === undefined ? null : p.reason } };
    }
    case "result": {
        var o = p.outcome;
        if (!isString(p.id) || !isObject(o) || !isObject(o.payload)) return fail("bad result");
        if (o._tag === "accepted") return { ok: true, msg: { _tag: "result", id: p.id, outcome: { _tag: "accepted" } } };
        if (o._tag === "rejected" && isString(o.payload.code) && isString(o.payload.message))
            return { ok: true, msg: { _tag: "result", id: p.id, outcome: { _tag: "rejected", code: o.payload.code, message: o.payload.message } } };
        return fail("bad result outcome");
    }
    case "error":
        if (!isString(p.code) || !isString(p.message)) return fail("bad error");
        return { ok: true, msg: { _tag: "error", code: p.code, message: p.message } };
    }
    return fail("unknown message " + m._tag);
}

function sameTarget(a, b) {
    return !!a && !!b && a.busName === b.busName && a.owner === b.owner;
}
