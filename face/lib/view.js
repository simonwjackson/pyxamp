.pragma library
// The face's one state adapter: daemon link + snapshot + local view state -> what
// the sections draw. Pure: every input is passed in, including the clock.
//
// Production and the fixture replay both reach this through the same socket
// link (DaemonLink.qml). Nothing here knows where a snapshot came from.
//
// input = {
//   link:       { _tag: "connecting" } | { _tag: "connected", instanceId } | { _tag: "disconnected", reason },
//   player:     decoded PlayerState (protocol.js) or null before the first snapshot,
//   observedAt: face monotonic ms when that snapshot arrived,
//   now:        face monotonic ms,
//   timeMode:   "elapsed" | "remaining",
//   flash:      { text, t0, until } | null   (local confirmations, ms),
//   seekDrag:   fraction 0..1 while the position thumb is held, else null,
//   marqueeSince: face monotonic ms when the marquee's base text last changed,
// }

var MARQUEE_CHARS = 31;

function pad2(n) { return (n < 10 ? "0" : "") + n; }

// 83 -> "1:23", 3723 -> "1:02:03"
function formatSeconds(s) {
    s = Math.max(0, Math.floor(s));
    var h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, x = s % 60;
    return h ? h + ":" + pad2(m) + ":" + pad2(x) : m + ":" + pad2(x);
}

function authority(input) {
    if (!input.link || input.link._tag !== "connected" || !input.player || input.player._tag !== "following") return null;
    return input.player.target;
}

// Observed position, moved on by local monotonic time only while playing.
function positionMs(input) {
    var p = input.player;
    if (!p || p._tag !== "following" || p.positionMs === undefined) return null;
    var pos = p.positionMs;
    if (p.transport === "playing") pos += Math.max(0, input.now - input.observedAt) * (p.rate === undefined ? 1 : p.rate);
    var d = p.track.durationMs;
    if (d !== undefined) pos = Math.min(pos, d);
    return Math.max(0, Math.floor(pos));
}

function lcd(input) {
    return input.link && input.link._tag === "connected" && input.player && input.player._tag === "following" ? "amber" : "green";
}

// Why a remote control cannot act right now, before any capability is asked.
function noAuthorityReason(input) {
    var l = input.link;
    if (!l || l._tag === "connecting") return "Connecting to the Pyxamp daemon";
    if (l._tag === "disconnected") return "The Pyxamp daemon is disconnected";
    var p = input.player;
    if (!p) return "Waiting for the daemon's first state";
    if (p._tag === "idle") return "No player to follow";
    if (p._tag === "unavailable") return "Players are unavailable: " + p.message;
    return "";
}

var CONTROL_CAP = { prev: "previous", play: "play", pause: "pause", stop: "stop", next: "next", seek: "seek", volume: "volume", repeat: "repeat", shuffle: "shuffle" };

function controls(input) {
    var out = {}, target = authority(input), base = noAuthorityReason(input);
    for (var name in CONTROL_CAP) {
        if (!target) { out[name] = { enabled: false, reason: base }; continue; }
        var p = input.player, cap = p.capabilities[CONTROL_CAP[name]], reason = "";
        if (!cap) reason = p.identity + " does not offer this";
        else if (name === "seek" && p.track.durationMs === undefined) reason = p.identity + " reports no track length";
        else if (name === "seek" && p.track.id === undefined) reason = p.identity + " reports no track id";
        else if (name === "seek" && p.positionMs === undefined) reason = p.identity + " reports no position";
        else if (name === "volume" && p.volume === undefined) reason = p.identity + " reports no volume";
        else if (name === "repeat" && p.repeat === undefined) reason = p.identity + " reports no repeat mode";
        else if (name === "shuffle" && p.shuffle === undefined) reason = p.identity + " reports no shuffle state";
        out[name] = { enabled: reason === "", reason: reason };
    }
    return out;
}

function trackLine(p) {
    var t = p.track, s = t.artist && t.title ? t.artist + " - " + t.title : (t.title || t.artist);
    if (t.durationMs !== undefined && s) s += " (" + formatSeconds(t.durationMs / 1000) + ")";
    return s;
}

// A scrolling line starts from its first character when its text changes, not
// on every snapshot: the caller passes marqueeSince, the time the text last changed.
function since(input) { return input.marqueeSince === undefined ? input.observedAt : input.marqueeSince; }

// The marquee is the status line: { text, fixed, t0, tip }.
function marquee(input) {
    var f = input.flash;
    if (f && input.now < f.until) return { text: f.text, fixed: true, t0: f.t0 };
    var l = input.link;
    if (!l || l._tag === "connecting") return { text: "CONNECTING TO THE PYXAMP DAEMON", fixed: true, t0: 0 };
    if (l._tag === "disconnected") return { text: "DAEMON DISCONNECTED", fixed: true, t0: 0, tip: "Daemon disconnected: " + l.reason + ". Reconnecting." };
    var p = input.player;
    if (!p) return { text: "CONNECTING TO THE PYXAMP DAEMON", fixed: true, t0: 0 };
    if (p._tag === "unavailable") return { text: "PLAYERS UNAVAILABLE: " + p.message, fixed: false, t0: since(input) };
    if (p._tag === "idle") return { text: "PYXAMP  ***  NO PLAYER TO FOLLOW. NO SOURCES YET", fixed: false, t0: since(input) };
    var line = trackLine(p);
    return { text: line ? p.identity + ": " + line : p.identity, fixed: false, t0: since(input) };
}

// "play" | "pause" | "stop" | "error" | "none"
function status(input) {
    var l = input.link;
    if (l && l._tag === "disconnected") return "error";
    if (!l || l._tag !== "connected" || !input.player) return "none";
    var p = input.player;
    if (p._tag === "unavailable") return "error";
    if (p._tag === "idle") return "stop";
    return { playing: "play", paused: "pause", stopped: "stop", unknown: "none" }[p.transport];
}

// The 9x13 clock: null when nothing can be shown, else digits.
function clock(input) {
    if (!authority(input)) return null;
    var p = input.player, pos = positionMs(input);
    if (pos === null || p.transport === "stopped") return null;
    var d = p.track.durationMs, remaining = input.timeMode === "remaining" && d !== undefined;
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
    if (!authority(input)) return null;
    var p = input.player, d = p.track.durationMs, pos = positionMs(input);
    if (!d || pos === null || p.transport === "stopped") return null;
    return Math.max(0, Math.min(1, pos / d));
}

// kbps/bit box, kHz box, mono/stereo. Absent metadata stays absent.
function format(input) {
    var out = { left: null, leftUnit: "kbps", khz: null, channels: 0 };
    if (!authority(input)) return out;
    var t = input.player.track;
    if (t.bitDepth !== undefined) { out.left = String(Math.round(t.bitDepth)); out.leftUnit = "bit"; }
    else if (t.bitrateKbps !== undefined && Math.round(t.bitrateKbps) <= 999) out.left = String(Math.round(t.bitrateKbps));
    if (t.sampleRateHz !== undefined) { var k = Math.round(t.sampleRateHz / 1000); if (k <= 999) out.khz = String(k); }
    if (t.channels !== undefined) out.channels = t.channels >= 2 ? 2 : t.channels === 1 ? 1 : 0;
    return out;
}

function eqNote(input) {
    var target = authority(input);
    return target ? ("NO EQ ON " + input.player.identity.toUpperCase()).slice(0, 22) : null;
}

// What the visualizer area shows:
//   { _tag: "label", text, mini }          VIS OFF under reduced motion, NO CAPTURE when capture failed
//   { _tag: "blank" }                      vis off, no daemon, no frame for this player yet
//   { _tag: "silent" }                     no signal: the spectrum is empty, the scope a flat line
//   { _tag: "signal", columns, waveform }  real samples for the followed player
function visual(input, frame, mode, reducedOff) {
    if (reducedOff) return { _tag: "label", text: "VIS OFF", mini: "OFF", tip: "Visualizer off (reduced motion). Pick a mode in the V menu." };
    if (mode === "off" || !input.link || input.link._tag !== "connected") return { _tag: "blank" };
    if (!frame) return { _tag: "blank" };
    var target = authority(input);
    var frameTarget = frame.target;
    var matches = target ? (frameTarget !== null && frameTarget.busName === target.busName && frameTarget.owner === target.owner) : frameTarget === null;
    if (!matches) return { _tag: "blank" };
    if (frame.status === "unavailable") {
        var who = target ? input.player.identity : "this player";
        return { _tag: "label", text: "NO CAPTURE", mini: "N/A", tip: "Visualizer: cannot capture " + who + (frame.reason ? " (" + frame.reason + ")" : "") };
    }
    if (frame.status === "idle") return { _tag: "silent" };
    return { _tag: "signal", columns: frame.columns, waveform: frame.waveform };
}

// Item info (I, Alt+3): the followed item's actual metadata, rows of [key, value].
function itemInfo(input) {
    if (!authority(input)) return { empty: noAuthorityReason(input) + "." , rows: [] };
    var p = input.player, t = p.track, rows = [["Player", p.identity]];
    if (t.title) rows.push(["Title", t.title]);
    if (t.artist) rows.push(["Artist", t.artist]);
    if (t.album !== undefined) rows.push(["Album", t.album]);
    if (t.durationMs !== undefined) rows.push(["Length", formatSeconds(t.durationMs / 1000)]);
    if (t.bitrateKbps !== undefined) rows.push(["Bitrate", Math.round(t.bitrateKbps) + " kbps"]);
    if (t.sampleRateHz !== undefined) rows.push(["Sample rate", (t.sampleRateHz / 1000).toFixed(t.sampleRateHz % 1000 ? 1 : 0) + " kHz"]);
    if (t.bitDepth !== undefined) rows.push(["Bit depth", t.bitDepth + " bit"]);
    if (t.channels !== undefined) rows.push(["Channels", t.channels === 1 ? "1 (mono)" : t.channels === 2 ? "2 (stereo)" : String(t.channels)]);
    if (p.rate !== undefined && p.rate !== 1) rows.push(["Speed", p.rate + "x"]);
    rows.push(["State", { playing: "playing", paused: "paused", stopped: "stopped", unknown: "unknown" }[p.transport]]);
    return { empty: "", rows: rows };
}

function main(input) {
    return {
        lcd: lcd(input),
        status: status(input),
        clock: clock(input),
        shortClock: shortClock(input),
        marquee: marquee(input),
        format: format(input),
        controls: controls(input),
        target: authority(input),
        volume: authority(input) && input.player.volume !== undefined ? input.player.volume : null,
        repeat: authority(input) && input.player.repeat !== undefined ? input.player.repeat : null,
        shuffle: authority(input) && input.player.shuffle !== undefined ? input.player.shuffle : null,
        position: positionFraction(input),
        positionMs: positionMs(input),
        eqNote: eqNote(input),
        identity: authority(input) ? input.player.identity : "",
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
