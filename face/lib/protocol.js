.pragma library
.import "wire-schema.js" as WireSchema
// Pyxamp control socket, version 1: the face's side of the wire.
// The daemon's Rust types are canonical; contracts/pyxamp.schema.json is
// generated from them, and lib/wire-schema.js from that schema
// (face/tools/gen-wire-validators). Every server line is validated there first.
// This module only builds client messages and adapts valid server messages to
// the face's tagged values: optional fields that are null or absent become
// absent, and nothing below the link reads raw JSON.

var VERSION = 1;
var MAX_LINE = 1048576; // 1 MiB, the same bound the daemon keeps

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

// Step 2: a source, library, own-playback or engine request. The daemon answers
// every request with one result; library.open and search first send a page.
// tag: "library.open", "own.transport", "eq.set" ... (docs/seam-step2.md).
function request(id, tag, payload) {
    return JSON.stringify({ _tag: "request", payload: { id: id, action: action(tag, payload) } });
}

// null if a client message is valid under the canonical schema, else "path: reason".
function checkClient(line) {
    var m;
    try { m = JSON.parse(line); } catch (e) { return "not JSON"; }
    return WireSchema.validateClientMessage(m);
}

// ---------- decoding: line -> { ok: true, msg } | { ok: false, reason } ----------

function fail(reason) { return { ok: false, reason: reason }; }
function present(v) { return v !== undefined && v !== null; }

// Copy the optional fields that are present (not absent, not null).
function optional(from, to, keys) {
    for (var i = 0; i < keys.length; i++) if (present(from[keys[i]])) to[keys[i]] = from[keys[i]];
    return to;
}

function target(t) { return { busName: t.busName, owner: t.owner }; }

function player(p) {
    var q = p.payload;
    if (p._tag === "idle") return { _tag: "idle" };
    if (p._tag === "unavailable") return { _tag: "unavailable", code: q.code, message: q.message };
    var track = optional(q.track, { title: q.track.title, artist: q.track.artist },
        ["id", "album", "artworkUrl", "durationMs", "bitrateKbps", "sampleRateHz", "bitDepth", "channels"]);
    var out = { _tag: "following", target: target(q.target), identity: q.identity, track: track, transport: q.transport, capabilities: q.capabilities };
    optional(q, out, ["positionMs", "volume", "repeat", "shuffle", "rate"]);
    if (out.volume !== undefined) out.volume = Math.max(0, Math.min(1, out.volume));
    return out;
}

function status(s) {
    return s._tag === "unavailable" ? { _tag: "unavailable", code: s.payload.code, message: s.payload.message } : { _tag: s._tag };
}

function item(i) {
    var out = optional(i, { source: i.source, id: i.id, kind: i.kind, title: i.title, openable: i.openable, playable: i.playable },
        ["subtitle", "album", "year", "durationMs", "count"]);
    if (present(i.progress)) out.progress = optional(i.progress, { played: i.progress.played }, ["positionMs"]);
    return out;
}

function library(l) {
    if (l._tag === "noSources") return { _tag: "noSources" };
    return { _tag: "sources", sources: l.sources.map(function (s) {
        return { source: s.source, label: s.label, status: status(s.status),
                 groups: s.groups.map(function (g) { return optional(g, { id: g.id, label: g.label, kind: g.kind }, ["count"]); }) };
    }) };
}

function output(o) { return { id: o.id, name: o.name, local: o.local, active: o.active }; }

function own(o) {
    if (!present(o)) return null;
    var track = optional(o.track, { title: o.track.title, artist: o.track.artist },
        ["id", "album", "artworkUrl", "durationMs", "bitrateKbps", "sampleRateHz", "bitDepth", "channels"]);
    var out = { source: o.source, output: output(o.output), track: track, transport: o.transport, capabilities: o.capabilities };
    optional(o, out, ["occurrence", "positionMs", "volume", "repeat", "shuffle", "speed", "note"]);
    if (present(o.skip)) out.skip = { backS: o.skip.backS, forwardS: o.skip.forwardS };
    if (present(o.retry)) out.retry = { attempt: o.retry.attempt, nextAtMs: o.retry.nextAtMs };
    if (present(o.yielded)) out.yielded = optional(o.yielded, {}, ["resumeByMs"]);
    return out;
}

function snapshot(p) {
    return {
        _tag: "state", revision: p.revision, player: player(p.player), library: library(p.library),
        queue: p.queue.map(function (q) { return optional(q, { occurrence: q.occurrence, item: item(q.item), status: q.status }, ["albumKey"]); }),
        own: own(p.own),
        outputs: (p.outputs || []).map(output),
        conflicts: (p.conflicts || []).map(function (c) {
            return { id: c.id, item: item(c.item), positions: c.positions.map(function (x) { return optional(x, { id: x.id, device: x.device, positionMs: x.positionMs }, ["updatedAtMs"]); }) };
        }),
        engine: present(p.engine) ? status(p.engine) : null,
    };
}

function page(p) {
    return optional(p, { _tag: "page", id: p.id, items: p.items.map(item) }, ["source", "node", "query", "nextCursor", "total"]);
}

function decodeServer(line) {
    if (line.length > MAX_LINE) return fail("line longer than 1 MiB");
    var m;
    try { m = JSON.parse(line); } catch (e) { return fail("not JSON"); }
    var error = WireSchema.validateServerMessage(m);
    if (error) return fail(error);
    var p = m.payload;
    switch (m._tag) {
    case "ready":
        if (p.version !== VERSION) return fail("the daemon speaks version " + p.version + ", this face " + VERSION);
        return { ok: true, msg: { _tag: "ready", instanceId: p.instanceId } };
    case "state":
        return { ok: true, msg: snapshot(p) };
    case "page":
        return { ok: true, msg: page(p) };
    case "visualizer":
        return { ok: true, msg: { _tag: "visualizer", target: present(p.target) ? target(p.target) : null, sequence: p.sequence, status: p.status,
                                  columns: p.columns, waveform: p.waveform, reason: present(p.reason) ? p.reason : null } };
    case "result":
        return { ok: true, msg: { _tag: "result", id: p.id, outcome: p.outcome._tag === "accepted"
            ? { _tag: "accepted" } : { _tag: "rejected", code: p.outcome.payload.code, message: p.outcome.payload.message } } };
    case "error":
        return { ok: true, msg: { _tag: "error", code: p.code, message: p.message } };
    }
    return fail("unknown message " + m._tag); // unreachable: the schema admits only the cases above
}

function sameTarget(a, b) {
    return !!a && !!b && a.busName === b.busName && a.owner === b.owner;
}
