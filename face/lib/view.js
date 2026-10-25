.pragma library
// The face's one state adapter for the main section: daemon link + snapshot +
// local view state -> what the sections draw. Pure: every input is passed in,
// including both clocks.
//
// Production and the fixture replay both reach this through the same socket
// link (DaemonLink.qml). Nothing here knows where a snapshot came from.
//
// What the main section shows (docs/seam-step2.md):
//   "following"  a foreign MPRIS player (snapshot.player), amber, step-1 commands;
//   "own"        Pyxamp's own session (snapshot.own), green, own.* requests;
//   "none"       neither.
//
// input = {
//   link:       { _tag: "connecting" } | { _tag: "connected", instanceId } | { _tag: "disconnected", reason },
//   player:     decoded PlayerState (protocol.js) or null before the first snapshot,
//   snap:       the decoded snapshot (protocol.js snapshot) or null,
//   observedAt: face monotonic ms when that snapshot arrived,
//   now:        face monotonic ms,
//   wallNow:    wall-clock ms, counted on from the snapshot's arrival on the monotonic clock,
//   timeMode:   "elapsed" | "remaining",
//   flash:      { text, t0, until } | null   (local confirmations, ms),
//   seekDrag:   fraction 0..1 while the position thumb is held, else null,
//   marqueeSince: face monotonic ms when the marquee's base text last changed,
// }

var MARQUEE_CHARS = 31;
var SOURCE_LABEL = { pyxis: "Pyxis", ossicle: "Ossicle", jellyfin: "Jellyfin" };
var SPEED_MIN = 0.5, SPEED_MAX = 3.0;

function pad2(n) { return (n < 10 ? "0" : "") + n; }

// 83 -> "1:23", 3723 -> "1:02:03"
function formatSeconds(s) {
    s = Math.max(0, Math.floor(s));
    var h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, x = s % 60;
    return h ? h + ":" + pad2(m) + ":" + pad2(x) : m + ":" + pad2(x);
}

function connected(input) { return !!input.link && input.link._tag === "connected"; }
function snapOf(input) { return input.snap || null; }

// "following" | "own" | "none"
function mode(input) {
    if (!connected(input) || !input.player) return "none";
    if (input.player._tag === "following") return "following";
    var s = snapOf(input);
    return s && s.own ? "own" : "none";
}

function authority(input) {
    return mode(input) === "following" ? input.player.target : null;
}

function own(input) { return mode(input) === "own" ? input.snap.own : null; }
// The own session even while a foreign player is followed (yield, the queue's "paused").
function session(input) { var s = snapOf(input); return connected(input) && s && s.own ? s.own : null; }

function sourceLabel(source) { return SOURCE_LABEL[source] || source; }
function spoken(o) { return !!o && o.source === "ossicle"; }
function consoleMode(o) { return !!o && !o.output.local; }
function conflictOpen(input) { var s = snapOf(input); return !!s && s.conflicts.length > 0; }

// The thing the clock, seek bar and format boxes describe.
function subject(input) {
    var m = mode(input);
    if (m === "following") { var p = input.player; return { track: p.track, transport: p.transport, positionMs: p.positionMs, rate: p.rate === undefined ? 1 : p.rate }; }
    if (m === "own") { var o = input.snap.own; return { track: o.track, transport: o.transport, positionMs: o.positionMs, rate: spoken(o) && o.speed !== undefined ? o.speed : 1, own: o }; }
    return null;
}

// Observed position, moved on by local monotonic time only while playing.
// Own playback hides it while a position conflict waits or a retry counts down.
function positionMs(input) {
    var s = subject(input);
    if (!s || s.positionMs === undefined) return null;
    if (s.own && (s.own.retry || conflictOpen(input))) return null;
    var pos = s.positionMs;
    if (s.transport === "playing") pos += Math.max(0, input.now - input.observedAt) * s.rate;
    var d = s.track.durationMs;
    if (d !== undefined) pos = Math.min(pos, d);
    return Math.max(0, Math.floor(pos));
}

function lcd(input) { return mode(input) === "following" ? "amber" : "green"; }

function hasSources(input) { var s = snapOf(input); return !!s && s.library._tag === "sources" && s.library.sources.length > 0; }

// Why a control cannot act right now, before any capability is asked.
function noAuthorityReason(input) {
    var l = input.link;
    if (!l || l._tag === "connecting") return "Connecting to the Pyxamp daemon";
    if (l._tag === "disconnected") return "The Pyxamp daemon is disconnected";
    var p = input.player;
    if (!p) return "Waiting for the daemon's first state";
    if (mode(input) !== "none") return "";
    if (p._tag === "unavailable") return "Players are unavailable: " + p.message;
    if (hasSources(input)) return "Nothing plays. Press J or L to find something";
    return "No player to follow";
}

// The volume slider is the system volume (the default audio sink), not a player's: see systemVolume().
var CONTROL_CAP = { prev: "previous", play: "play", pause: "pause", stop: "stop", next: "next", seek: "seek", repeat: "repeat", shuffle: "shuffle" };

// The system volume, in every state: { volume (0..1, for the slider), real (may exceed 1), muted } or null.
function systemVolume(input) {
    var s = snapOf(input);
    if (!connected(input) || !s || !s.systemVolume) return null;
    var real = s.systemVolume.volume;
    return { volume: Math.max(0, Math.min(1, real)), real: real, muted: s.systemVolume.muted };
}

// The volume slider's tooltip: the real value (a boost shows above 100 %).
function volumeTip(sys, control) {
    if (!sys) return "System volume: " + control.reason;
    return "System volume: " + Math.round(sys.real * 100) + "%" + (sys.muted ? " (muted)" : "");
}

// The value Up or Down (delta) sends, or null for none. Up from a boost above
// 100 % would lower the volume to 100 %, so it sends nothing.
function volumeStep(sys, delta) {
    if (!sys) return null;
    if (delta > 0 && sys.real >= 1) return null;
    return Math.round(Math.max(0, Math.min(1, sys.volume + delta)) * 100) / 100;
}

function controls(input) {
    var out = {}, m = mode(input), base = noAuthorityReason(input);
    var holder = m === "following" ? input.player : m === "own" ? input.snap.own : null;
    var name0 = m === "following" ? holder.identity : m === "own" ? sourceLabel(holder.source) : "";
    for (var name in CONTROL_CAP) {
        if (!holder) { out[name] = { enabled: false, reason: base }; continue; }
        var cap = holder.capabilities[CONTROL_CAP[name]], reason = "", t = holder.track;
        if (m === "own" && spoken(holder) && (name === "repeat" || name === "shuffle")) reason = (name === "repeat" ? "Repeat" : "Shuffle") + " is not for spoken word";
        else if (!cap) reason = name0 + " does not offer this";
        else if (name === "seek" && t.durationMs === undefined) reason = name0 + " reports no track length";
        else if (name === "seek" && m === "following" && t.id === undefined) reason = name0 + " reports no track id";
        else if (name === "seek" && m === "own" && holder.occurrence === undefined) reason = name0 + " reports no queue position";
        else if (name === "seek" && holder.positionMs === undefined) reason = name0 + " reports no position";
        else if (name === "seek" && m === "own" && conflictOpen(input)) reason = "Pick a position first: POSITION CONFLICT";
        else if (name === "repeat" && holder.repeat === undefined) reason = name0 + " reports no repeat mode";
        else if (name === "shuffle" && holder.shuffle === undefined) reason = name0 + " reports no shuffle state";
        out[name] = { enabled: reason === "", reason: reason };
    }
    // The system volume works whatever plays, also while nothing does.
    var l = input.link;
    if (systemVolume(input)) out.volume = { enabled: true, reason: "" };
    else if (!l || l._tag !== "connected" || !input.snap) out.volume = { enabled: false, reason: base };
    else out.volume = { enabled: false, reason: "The system volume is unknown (no PipeWire default sink)" };
    // Spoken word: the balance slider is the speed slider (0.5x to 3.0x).
    var o = own(input);
    if (o && spoken(o)) out.speed = o.speed !== undefined ? { enabled: true, reason: "" } : { enabled: false, reason: "Ossicle reports no speed" };
    else out.speed = { enabled: false, reason: o ? "Speed is for spoken word" : base || "Speed is for Pyxamp's own spoken word" };
    // Balance changes Pyxamp's own audio here, so not on a remote output or a foreign player.
    if (o && !spoken(o) && !consoleMode(o)) out.balance = { enabled: true, reason: "" };
    else if (o && consoleMode(o)) out.balance = { enabled: false, reason: "Balance: Pyxamp's own audio only, not " + o.output.name };
    else if (m === "following") out.balance = { enabled: false, reason: "Balance: Pyxamp's own audio only, not " + input.player.identity };
    else out.balance = { enabled: false, reason: base || "Balance: Pyxamp's own audio only; nothing plays" };
    return out;
}

function trackLine(t) {
    var s = t.artist && t.title ? t.artist + " - " + t.title : (t.title || t.artist);
    if (t.durationMs !== undefined && s) s += " (" + formatSeconds(t.durationMs / 1000) + ")";
    return s;
}

// The 1-based queue number of the own occurrence, or 0.
function queueNumber(input, o) {
    if (!o || o.occurrence === undefined) return 0;
    var q = input.snap.queue;
    for (var i = 0; i < q.length; i++) if (q[i].occurrence === o.occurrence) return i + 1;
    return 0;
}

// Seconds until a wall-clock deadline, at least 0.
function secondsTo(input, atMs) { return Math.max(0, Math.ceil((atMs - (input.wallNow || 0)) / 1000)); }

// While a foreign player is followed and own playback yielded, the tail says
// the resume rule (brief: "RESUMES IF THIS STOPS BY 1:26" or "STAYS PAUSED").
function yieldTail(input) {
    var o = session(input);
    if (!o || !o.yielded) return null;
    if (o.yielded.resumeByMs === undefined) return { text: "PYXAMP STAYS PAUSED", key: "stays" };
    var left = secondsTo(input, o.yielded.resumeByMs);
    if (left <= 0) return { text: "PYXAMP STAYS PAUSED", key: "stays" };
    return { text: "PYXAMP PAUSED, RESUMES IF THIS STOPS BY " + formatSeconds(left), key: "resumes" };
}

// A scrolling line starts from its first character when its text changes, not
// on every snapshot: the caller passes marqueeSince, the time its key last changed.
function since(input) { return input.marqueeSince === undefined ? input.observedAt : input.marqueeSince; }

function ownMarquee(input, o) {
    var s = input.snap;
    if (s.conflicts.length) return { text: "POSITION CONFLICT", fixed: true, key: "conflict" };
    if (o.retry) {
        var left = secondsTo(input, o.retry.nextAtMs);
        return { text: left > 0 ? "CANNOT PLAY: RETRY " + o.retry.attempt + " IN " + left + " S" : "CANNOT PLAY: RETRYING", fixed: true, key: "retry" };
    }
    if (o.output.local && s.engine && s.engine._tag === "unavailable")
        return { text: "PLAYBACK UNAVAILABLE: " + s.engine.message, fixed: false, key: "engine:" + s.engine.message };
    if (o.note) return { text: o.note, fixed: true, key: "note:" + o.note };
    var n = queueNumber(input, o), line = trackLine(o.track) || sourceLabel(o.source);
    if (n) line = n + ". " + line;
    if (consoleMode(o)) line = o.output.name + ": " + line;
    return { text: line, fixed: false, key: line };
}

// The marquee is the status line: { text, fixed, t0, tip, key }.
function marquee(input) {
    var f = input.flash, out;
    if (f && input.now < f.until) return { text: f.text, fixed: true, t0: f.t0, key: "flash" };
    var l = input.link;
    if (!l || l._tag === "connecting") return { text: "CONNECTING TO THE PYXAMP DAEMON", fixed: true, t0: 0, key: "connecting" };
    if (l._tag === "disconnected") return { text: "DAEMON DISCONNECTED", fixed: true, t0: 0, tip: "Daemon disconnected: " + l.reason + ". Reconnecting.", key: "lost" };
    var p = input.player;
    if (!p) return { text: "CONNECTING TO THE PYXAMP DAEMON", fixed: true, t0: 0, key: "connecting" };
    var m = mode(input);
    if (m === "following") {
        var line = trackLine(p.track), base = line ? p.identity + ": " + line : p.identity, tail = yieldTail(input);
        out = tail ? { text: base + "  ***  " + tail.text, fixed: false, key: base + "|" + tail.key } : { text: base, fixed: false, key: base };
    } else if (m === "own") {
        out = ownMarquee(input, input.snap.own);
    } else if (p._tag === "unavailable") {
        out = { text: "PLAYERS UNAVAILABLE: " + p.message, fixed: false, key: "unavailable" };
    } else if (hasSources(input)) {
        out = { text: "PYXAMP  ***  NOTHING PLAYS. PRESS J OR L TO FIND SOMETHING", fixed: false, key: "nothing" };
    } else {
        out = { text: "PYXAMP  ***  NO PLAYER TO FOLLOW. NO SOURCES YET", fixed: false, key: "idle" };
    }
    out.t0 = since(input);
    return out;
}

// "play" | "pause" | "stop" | "error" | "none"
function status(input) {
    var l = input.link;
    if (l && l._tag === "disconnected") return "error";
    if (!l || l._tag !== "connected" || !input.player) return "none";
    var m = mode(input), p = input.player;
    if (m === "own") {
        var o = input.snap.own;
        if (o.retry) return "error";
        if (o.output.local && input.snap.engine && input.snap.engine._tag === "unavailable") return "error";
        return { playing: "play", paused: "pause", stopped: "stop", unknown: "none" }[o.transport];
    }
    if (p._tag === "unavailable") return "error";
    if (m === "none") return "stop";
    return { playing: "play", paused: "pause", stopped: "stop", unknown: "none" }[p.transport];
}

// The 9x13 clock: null when nothing can be shown, else digits.
function clock(input) {
    var s = subject(input), pos = positionMs(input);
    if (!s || pos === null || s.transport === "stopped") return null;
    var d = s.track.durationMs, remaining = input.timeMode === "remaining" && d !== undefined;
    var total = Math.floor((remaining ? Math.max(0, d - pos) : pos) / 1000), hours = total >= 6000, a, b;
    if (hours) { a = Math.floor(total / 3600); b = Math.floor(total / 60) % 60; } else { a = Math.floor(total / 60); b = total % 60; }
    return { minus: remaining, digits: [Math.floor(a / 10) % 10, a % 10, Math.floor(b / 10), b % 10], hours: hours };
}

// "1:23" text for the windowshade and the playlist's mini clock (6 characters).
// Like the big clock, past 99:59 it shows hours and minutes with an H ("6:02H").
function shortClock(input) {
    var c = clock(input);
    if (!c) return "";
    var a = c.digits[0] * 10 + c.digits[1], b = c.digits[2] * 10 + c.digits[3];
    return (c.minus ? "-" : "") + a + ":" + pad2(b) + (c.hours ? "H" : "");
}

function positionFraction(input) {
    if (input.seekDrag !== null && input.seekDrag !== undefined) return input.seekDrag;
    var s = subject(input), pos = positionMs(input);
    if (!s) return null;
    var d = s.track.durationMs;
    if (!d || pos === null || s.transport === "stopped") return null;
    return Math.max(0, Math.min(1, pos / d));
}

// kbps/bit box, kHz box, mono/stereo. Absent metadata stays absent.
function format(input) {
    var out = { left: null, leftUnit: "kbps", khz: null, channels: 0 }, s = subject(input);
    if (!s) return out;
    if (s.own && (s.own.retry || s.transport === "stopped")) return out;
    var t = s.track;
    if (t.bitDepth !== undefined) { out.left = String(Math.round(t.bitDepth)); out.leftUnit = "bit"; }
    else if (t.bitrateKbps !== undefined && Math.round(t.bitrateKbps) <= 999) out.left = String(Math.round(t.bitrateKbps));
    if (t.sampleRateHz !== undefined) { var k = Math.round(t.sampleRateHz / 1000); if (k <= 999) out.khz = String(k); }
    if (t.channels !== undefined) out.channels = t.channels >= 2 ? 2 : t.channels === 1 ? 1 : 0;
    return out;
}

// The EQ changes Pyxamp's own audio only: the graph says so while it does nothing.
function eqNote(input) {
    var m = mode(input);
    if (m === "following") return ("NO EQ ON " + input.player.identity.toUpperCase()).slice(0, 22);
    var o = own(input);
    if (o && consoleMode(o)) return ("NO EQ ON " + o.output.name.toUpperCase()).slice(0, 22);
    return null;
}

// What the visualizer area shows:
//   { _tag: "label", text, mini }          VIS OFF under reduced motion, NO CAPTURE when capture failed
//   { _tag: "blank" }                      vis off, no daemon, no frame for this player yet
//   { _tag: "flat" }                       console mode: no audio here, a flat line in every mode
//   { _tag: "silent" }                     no signal: the spectrum is empty, the scope a flat line
//   { _tag: "signal", columns, waveform }  real samples for the followed player
function visual(input, frame, visMode, reducedOff) {
    if (reducedOff) return { _tag: "label", text: "VIS OFF", mini: "OFF", tip: "Visualizer off (reduced motion). Pick a mode in the V menu." };
    if (visMode === "off" || !connected(input)) return { _tag: "blank" };
    var o = own(input);
    if (o && consoleMode(o)) return { _tag: "flat", tip: "Plays on " + o.output.name + ": no audio here to show" };
    if (o && o.retry) return { _tag: "blank" };
    if (!frame) return { _tag: "blank" };
    var target = authority(input);
    var frameTarget = frame.target;
    var matches = target ? (frameTarget !== null && frameTarget.busName === target.busName && frameTarget.owner === target.owner) : frameTarget === null;
    if (!matches) return { _tag: "blank" };
    if (frame.status === "unavailable") {
        var who = target ? input.player.identity : o ? "Pyxamp's own audio" : "this player";
        return { _tag: "label", text: "NO CAPTURE", mini: "N/A", tip: "Visualizer: cannot capture " + who + (frame.reason ? " (" + frame.reason + ")" : "") };
    }
    if (frame.status === "idle") return { _tag: "silent" };
    return { _tag: "signal", columns: frame.columns, waveform: frame.waveform };
}

function trackRows(t, rows, spokenWord) {
    if (t.title) rows.push(["Title", t.title]);
    if (t.artist) rows.push([spokenWord ? "Show" : "Artist", t.artist]);
    if (t.album !== undefined) rows.push(["Album", t.album]);
    if (t.durationMs !== undefined) rows.push(["Length", formatSeconds(t.durationMs / 1000)]);
    if (t.bitrateKbps !== undefined) rows.push(["Bitrate", Math.round(t.bitrateKbps) + " kbps"]);
    if (t.sampleRateHz !== undefined) rows.push(["Sample rate", (t.sampleRateHz / 1000).toFixed(t.sampleRateHz % 1000 ? 1 : 0) + " kHz"]);
    if (t.bitDepth !== undefined) rows.push(["Bit depth", t.bitDepth + " bit"]);
    if (t.channels !== undefined) rows.push(["Channels", t.channels === 1 ? "1 (mono)" : t.channels === 2 ? "2 (stereo)" : String(t.channels)]);
}
var STATE_WORD = { playing: "playing", paused: "paused", stopped: "stopped", unknown: "unknown" };

// Item info (I, Alt+3): the shown item's actual metadata, rows of [key, value].
function itemInfo(input) {
    var m = mode(input), rows;
    if (m === "following") {
        var p = input.player;
        rows = [["Player", p.identity]];
        trackRows(p.track, rows, false);
        if (p.rate !== undefined && p.rate !== 1) rows.push(["Speed", p.rate + "x"]);
        rows.push(["State", STATE_WORD[p.transport]]);
        return { empty: "", rows: rows };
    }
    if (m === "own") {
        var o = input.snap.own, sw = spoken(o), pos = positionMs(input);
        rows = [["Source", sourceLabel(o.source) + (sw ? " (spoken word)" : o.source === "pyxis" ? (consoleMode(o) ? " (console)" : " (host)") : "")]];
        rows.push(["Plays on", o.output.local ? o.output.name + " (here)" : o.output.name]);
        trackRows(o.track, rows, sw);
        if (pos !== null) rows.push(["Position", formatSeconds(pos / 1000) + (sw ? " (synced)" : "")]);
        if (o.speed !== undefined) rows.push(["Speed", o.speed.toFixed(1) + "x" + (sw ? " (synced)" : "")]);
        if (o.skip) rows.push(["Skip", "back " + o.skip.backS + " s, forward " + o.skip.forwardS + " s"]);
        var q = input.snap.queue;
        for (var i = 0; i < q.length; i++) if (q[i].occurrence === o.occurrence && q[i].item.progress) { rows.push(["Played", q[i].item.progress.played ? "yes" : "no"]); break; }
        if (o.retry) rows.push(["Retry", "attempt " + o.retry.attempt]);
        rows.push(["State", STATE_WORD[o.transport]]);
        return { empty: "", rows: rows };
    }
    return { empty: noAuthorityReason(input) + ".", rows: [] };
}

// The confirmation for a change between two snapshots, or null: the foreign
// player went away and own playback comes back (brief: "BACK TO PYXAMP:
// RESUMING" or "STAYS PAUSED" for 2 s), or a track was given up.
function transition(before, after) {
    if (!before || !after) return null;
    var wasFollowing = before.player && before.player._tag === "following";
    var isFollowing = after.player && after.player._tag === "following";
    if (wasFollowing && !isFollowing && before.own && before.own.yielded && after.own)
        return after.own.transport === "playing" ? "BACK TO PYXAMP: RESUMING" : "BACK TO PYXAMP: STAYS PAUSED";
    // A track given up: the same occurrence of the same source turned failed.
    if (!before.own || !after.own || before.own.source !== after.own.source) return null;
    for (var j = 0; j < after.queue.length; j++) {
        var q = after.queue[j];
        if (q.status !== "failed") continue;
        for (var i = 0; i < before.queue.length; i++) {
            var b = before.queue[i];
            if (b.occurrence === q.occurrence && b.item.id === q.item.id && b.status !== "failed") return "SKIPPED: TRACK CANNOT PLAY";
        }
    }
    return null;
}

function main(input) {
    var m = mode(input), o = own(input), sw = spoken(o), s = subject(input), sysVol = systemVolume(input), ctl = controls(input);
    var holder = m === "following" ? input.player : o;
    return {
        mode: m,
        lcd: lcd(input),
        status: status(input),
        clock: clock(input),
        shortClock: shortClock(input),
        marquee: marquee(input),
        format: format(input),
        controls: ctl,
        target: authority(input),
        own: o,
        source: o ? o.source : "",
        sourceLabel: o ? sourceLabel(o.source) : "",
        occurrence: o && o.occurrence !== undefined ? o.occurrence : "",
        spoken: sw,
        console: consoleMode(o),
        skip: sw && o.skip ? o.skip : null,
        speed: sw && o.speed !== undefined ? o.speed : null,
        paused: !!s && s.transport === "paused",
        durationMs: s && s.track.durationMs !== undefined ? s.track.durationMs : null,
        trackId: m === "following" && input.player.track.id !== undefined ? input.player.track.id : "",
        volume: sysVol ? sysVol.volume : null,
        volumeReal: sysVol ? sysVol.real : null,
        muted: sysVol ? sysVol.muted : null,
        volumeTip: volumeTip(sysVol, ctl.volume),
        repeat: holder && holder.repeat !== undefined && !sw ? holder.repeat : null,
        shuffle: holder && holder.shuffle !== undefined && !sw ? holder.shuffle : null,
        position: positionFraction(input),
        positionMs: positionMs(input),
        eqNote: eqNote(input),
        eqApplies: !!o && !consoleMode(o),
        identity: m === "following" ? input.player.identity : o ? (consoleMode(o) ? o.output.name : sourceLabel(o.source)) : "",
        conflict: conflictOpen(input),
    };
}

// The 5x6 font has upper case only; accents fall back to their base letter.
function toText5(s) {
    var out = String(s).toUpperCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "");
    return out.replace(/[\u2018\u2019]/g, "'").replace(/[\u201c\u201d]/g, '"').replace(/[\u2013\u2014]/g, "-").replace(/\u00d7/g, "X");
}

// Marquee frame: the visible text and its x offset (skin pixels) at time now.
// Scrolls 5 sp per 220 ms when longer than 31 characters, unless reduced motion.
function marqueeFrame(text, fixed, t0, now, reduced) {
    var t = toText5(text);
    if (t.length <= MARQUEE_CHARS) return { text: t, offset: 0 };
    if (reduced) return { text: t.slice(0, MARQUEE_CHARS - 3) + "...", offset: 0 };
    var loop = t + "  ***  ", lw = loop.length * 5;
    var off = (Math.floor(Math.max(0, now - t0) / 220) * 5) % lw;
    return { text: loop + loop, offset: off };
}

// Speed slider (spoken word): 0.5x to 3.0x in 0.1 steps over 24 sp of travel.
function speedAt(fraction) { return Math.round((SPEED_MIN + Math.max(0, Math.min(1, fraction)) * (SPEED_MAX - SPEED_MIN)) * 10) / 10; }
function speedFraction(speed) { return (Math.max(SPEED_MIN, Math.min(SPEED_MAX, speed)) - SPEED_MIN) / (SPEED_MAX - SPEED_MIN); }
// Balance: -1 (left) .. 1 (right), snapped to centre near 0, as in the mock.
function balanceAt(fraction) { var b = Math.max(0, Math.min(1, fraction)) * 2 - 1; return Math.abs(b) < 0.12 ? 0 : Math.round(b * 100) / 100; }
function balanceText(b) { return b === 0 ? "BALANCE: CENTER" : "BALANCE: " + Math.round(Math.abs(b) * 100) + "% " + (b < 0 ? "LEFT" : "RIGHT"); }
