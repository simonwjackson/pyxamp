import QtQuick
import Quickshell
import Quickshell.Hyprland
import "lib/view.js" as View
import "lib/playlist.js" as Playlist
import "lib/fit.js" as Fit
import "lib/vis.js" as Vis
import "lib/keys.js" as Keys
import "lib/eq.js" as Eq
import "lib/protocol.js" as Protocol

// The composition root of the face: it reads the daemon link and the local
// preferences, derives the view models once (lib/view.js for the main section,
// lib/playlist.js for the playlist section), owns the local view state and
// every action, and hands one object to the sections below. Nothing below this
// file reads the link or the preferences directly.
//
// What plays decides where a control goes (docs/seam-step2.md): while a foreign
// player is followed (amber) the step-1 commands go to it; while Pyxamp's own
// session is shown (green) the own.* requests go to the active source. The
// volume slider is the exception: it is the system volume in every state.
Scope {
    id: face
    required property DaemonLink link
    required property Prefs prefs
    required property MonoClock clock

    // ---------- local view state ----------
    property string view: "queue"           // queue | library | jump | info (the playlist section)
    property var flash: null                // { text, t0, until }
    property real seekDrag: -1              // 0..1 while the position thumb is held
    property real volumeDrag: -1            // 0..1 while the volume knob is held
    property real speedDrag: -1             // 0.5..3.0 while the speed knob is held
    property real balanceDrag: -2           // -1..1 while the balance knob is held
    property bool visChosen: false          // reduced motion: a vis mode was picked this session
    property string menu: ""                // open popup menu
    property Item menuAnchor: null
    property var menuRow: null              // the row a context menu acts on (its ref)
    property var gesture: null              // what a pointer gesture began on
    property var pending: ({})              // request or command id -> { label, ok, silent, lib, jump }
    property real now: clock.ms
    property ShellScreen screen: null       // where the surface is (set on show)
    property var dragPos: null              // live position while the title bar is dragged

    // Playlist section state. It lives here, so a fold, a resize or a view
    // change keeps it: selection, the library path and pages, the jump text.
    property string sel: ""                 // selected queue occurrence
    property int albumSel: 0
    property string albumOpen: ""           // the opened album's first occurrence
    property int conflictPick: 0
    property var lib: ({ path: [], sel: 0 })
    property var libPages: ({})             // source + node -> { status, items, nextCursor, total, message, loadingMore }
    property var jump: ({ text: "", sel: 0, status: "idle", items: [], message: "", reqId: "" })
    property var prompt: null               // { kind, title, label, text, right, hint, refold }
    readonly property string jumpText: face.jump.text

    readonly property var p: prefs.value
    readonly property bool reducedOff: p.reducedMotion && !face.visChosen
    readonly property real wallNow: link.observedWall + (face.now - link.observedAt)
    readonly property var input: ({
        link: link.link, player: link.player, snap: link.snap, observedAt: link.observedAt, now: face.now, wallNow: face.wallNow,
        timeMode: p.timeMode, flash: face.flash, seekDrag: face.seekDrag >= 0 ? face.seekDrag : null,
        marqueeSince: face.marqueeSince,
    })
    // The marquee restarts its scroll only when its text changes (a countdown does not count).
    property real marqueeSince: 0
    readonly property string marqueeKey: View.marquee({ link: link.link, player: link.player, snap: link.snap, observedAt: 0, now: 0, wallNow: face.wallNow, flash: null }).key
    onMarqueeKeyChanged: Qt.callLater(() => { face.marqueeSince = face.clock.now(); })
    readonly property var m: View.main(face.input)
    readonly property var visual: View.visual(face.input, link.frame, p.vis, face.reducedOff)
    // Item info changes with the position, so it is worked out only while it shows.
    readonly property string infoJson: face.view === "info" ? JSON.stringify(View.itemInfo(Object.assign({}, face.input, { now: face.observedSecond }))) : ""
    readonly property real observedSecond: link.observedAt + Math.floor((face.now - link.observedAt) / 1000) * 1000
    readonly property var info: face.infoJson ? JSON.parse(face.infoJson) : ({ empty: "", rows: [] })
    // The playlist model must not depend on the clock: its rows are rebuilt only when
    // what they show changes (a new snapshot, a key, a page).
    readonly property string shownMode: View.mode({ link: link.link, player: link.player, snap: link.snap })
    readonly property var plCtx: ({
        snap: link.snap && link.link._tag === "connected" ? link.snap : null, main: ({ mode: face.shownMode }), view: face.view,
        albumMode: p.albumMode, albumOpen: face.albumOpen, albumSel: face.albumSel, sel: face.sel,
        lib: face.lib, pages: face.libPages, jump: face.jump, prompt: face.prompt, conflictPick: face.conflictPick,
        info: face.info, caret: "_",
    })
    readonly property var pl: Playlist.model(face.plCtx)
    readonly property var activeSummary: Playlist.summary(face.plCtx, face.m.source)
    // The EQ the engine gets: AUTO picks Voice for spoken word (decision 19).
    readonly property var eqShown: Eq.effective(p.eq, face.m.spoken || (link.snap && link.snap.own ? link.snap.own.source === "ossicle" : false))

    // ---------- monitor, scale, fit ----------
    function focusedScreen() {
        const mon = Hyprland.focusedMonitor;
        if (mon) for (const s of Quickshell.screens) if (s.name === mon.name) return s;
        return Quickshell.screens.length ? Quickshell.screens[0] : null;
    }
    // The compositor's scale for the monitor. Qt rounds a fractional output scale
    // up in the screen's devicePixelRatio (1.5 reads as 2), so ask Hyprland first.
    readonly property real monitorScale: {
        const s = face.screen;
        if (!s) return 1;
        const hm = Hyprland.monitorFor(s);
        return hm && hm.scale > 0 ? hm.scale : s.devicePixelRatio;
    }
    function planFor(wantDbl) {
        const s = face.screen, r = face.monitorScale;
        return Fit.plan({ width: s ? Math.round(s.width * r) : 1920, height: s ? Math.round(s.height * r) : 1080, scale: r },
                        { dbl: wantDbl, eq: p.eqOpen, pl: p.plOpen, shade: p.shade, plN: p.plN });
    }
    readonly property var fit: planFor(p.dbl)
    readonly property var place: face.dragPos ? face.dragPos
        : Fit.place(face.screen ? p.positions[face.screen.name] : undefined, { w: fit.logicalW, h: fit.logicalH },
                    { width: face.screen ? face.screen.width : 1920, height: face.screen ? face.screen.height : 1080 })

    // ---------- visualizer display state (bars fall, peaks hold) ----------
    property var visState: Vis.empty()
    readonly property bool visMoving: face.visual._tag === "signal" || !Vis.isQuiet(face.visState)
    Timer {
        interval: Vis.TICK_MS
        repeat: true
        running: face.p.shown && face.visMoving
        onTriggered: face.visState = Vis.step(face.visState, face.visual._tag === "signal" ? Vis.bands(face.visual.columns) : null)
    }
    onVisualChanged: if (face.visual._tag !== "signal" && face.visual._tag !== "silent") face.visState = Vis.empty()
    // The clock runs while time matters: playing (even hidden, so the position is
    // right when it shows), a countdown on the display, a confirmation, and motion on screen.
    readonly property bool flashing: face.flash !== null && face.now < face.flash.until
    readonly property bool countingDown: !!(link.snap && link.snap.own && (link.snap.own.retry || (link.snap.own.yielded && link.snap.own.yielded.resumeByMs !== undefined)))
    Binding {
        target: face.clock; property: "animating"
        value: face.m.status === "play" || face.flashing || face.countingDown || (face.p.shown && (face.visMoving || (!face.p.reducedMotion && face.m.marquee.text.length > View.MARQUEE_CHARS)))
    }

    // ---------- feedback ----------
    function say(text, ms) { face.flash = { text: text, t0: face.clock.now(), until: face.clock.now() + (ms || 1600) }; }
    function refuse(label, control) { face.say(label + ": " + control.reason, 2400); }
    function upper(s) { return String(s).toUpperCase(); }

    // ---------- the daemon's answers ----------
    function forget(id) { const next = Object.assign({}, face.pending); delete next[id]; face.pending = next; }
    function setPage(key, entry) { const next = Object.assign({}, face.libPages); next[key] = entry; face.libPages = next; }
    Connections {
        target: face.link
        function onOutcome(id, result) {
            const job = face.pending[id];
            if (job === undefined) return;
            face.forget(id);
            if (job.lib) {
                const pg = face.libPages[job.lib.key];
                if (result._tag === "rejected" && pg) {
                    if (job.lib.append) face.setPage(job.lib.key, Object.assign({}, pg, { loadingMore: false }));
                    else face.setPage(job.lib.key, { status: "error", items: [], message: result.message });
                } else if (pg && pg.status === "loading") face.setPage(job.lib.key, { status: "ready", items: [] });
            }
            if (job.jump && id === face.jump.reqId && result._tag === "rejected")
                face.jump = Object.assign({}, face.jump, { status: "error", message: result.message });
            if (result._tag === "rejected") { if (!job.silent) face.say(job.label + " REFUSED: " + result.message, 3000); }
            else if (job.ok) face.say(job.ok, job.okMs || 2000);
        }
        function onPage(page) {
            const job = face.pending[page.id];
            if (!job) return;
            if (job.lib) {
                const pg = face.libPages[job.lib.key];
                const items = job.lib.append && pg && pg.items ? pg.items.concat(page.items) : page.items;
                const entry = { status: "ready", items: items };
                if (page.nextCursor !== undefined) entry.nextCursor = page.nextCursor;
                if (page.total !== undefined) entry.total = page.total;
                face.setPage(job.lib.key, entry);
            }
            if (job.jump && page.id === face.jump.reqId)
                face.jump = Object.assign({}, face.jump, { status: "ready", items: page.items, sel: 0 });
        }
        function onLinkChanged() {
            if (face.link.link._tag !== "connected") {
                face.pending = ({}); face.seekDrag = -1; face.volumeDrag = -1; face.speedDrag = -1; face.balanceDrag = -2;
                face.libPages = ({}); face.lastSnap = null;
                if (face.jump.status === "searching") face.jump = Object.assign({}, face.jump, { status: "idle" });
            } else {
                Qt.callLater(() => face.syncEngine(true));
            }
        }
        // After the bindings settle: a confirmation or a tidy-up changes their inputs.
        function onSnapChanged() { const s = face.link.snap; Qt.callLater(() => face.afterSnap(s)); }
    }
    property var lastSnap: null
    function afterSnap(s) {
        const said = View.transition(face.lastSnap, s);
        if (said) face.say(said, 2000);
        face.lastSnap = s;
        face.tidy();
    }

    // Keep local selections valid for the queue the daemon now reports.
    function tidy() {
        const s = face.link.snap;
        if (!s) return;
        if (face.sel && !s.queue.some((q) => q.occurrence === face.sel)) face.sel = "";
        if (face.albumOpen && !s.queue.some((q) => q.occurrence === face.albumOpen)) face.albumOpen = "";
        if (!s.conflicts.length) face.conflictPick = 0;
    }

    // ---------- sending ----------
    // A step-1 command to the followed player; a gesture keeps the target it began with.
    function beginGesture() {
        face.gesture = { mode: face.m.mode, target: face.m.target, trackId: face.m.trackId, occurrence: face.m.occurrence };
    }
    function gestureOr(g) { return g || { mode: face.m.mode, target: face.m.target, trackId: face.m.trackId, occurrence: face.m.occurrence }; }
    function send(label, target, tag, payload) {
        if (!target) return;
        const id = face.link.command(target, Protocol.action(tag, payload));
        if (id) { const next = Object.assign({}, face.pending); next[id] = { label: label }; face.pending = next; }
    }
    // A step-2 request. job: { ok: text on acceptance, silent, lib, jump }.
    function request(label, tag, payload, job) {
        const id = face.link.request(tag, payload);
        if (!id) { if (!(job && job.silent)) face.say(label + ": " + View.noAuthorityReason(face.input).replace(/^$/, "The Pyxamp daemon is not connected"), 2400); return ""; }
        const next = Object.assign({}, face.pending); next[id] = Object.assign({ label: label }, job || {}); face.pending = next;
        return id;
    }

    readonly property var transportTags: ({ prev: "transport.previous", play: "transport.play", pause: "transport.pause", stop: "transport.stop", next: "transport.next" })
    readonly property var ownTransport: ({ prev: "previous", play: "play", pause: "pause", stop: "stop", next: "next" })
    readonly property var transportLabels: ({ prev: "PREVIOUS", play: "PLAY", pause: "PAUSE", stop: "STOP", next: "NEXT" })

    // which: prev | play | pause | stop | next. g: the gesture, or now.
    function transport(which, g) {
        g = face.gestureOr(g);
        let w = which;
        // C and the pause button toggle, as in Winamp: paused -> play.
        if (w === "pause" && face.m.paused) w = "play";
        const c = face.m.controls[w];
        if (!c.enabled) { face.refuse(face.transportLabels[w], c); return; }
        if (g.mode === "own") {
            // Spoken word: Prev and Next skip by Ossicle's intervals (the arrows keep 5 s).
            if (face.m.skip && (w === "prev" || w === "next")) {
                const s = w === "prev" ? -face.m.skip.backS : face.m.skip.forwardS;
                if (face.seekBy(s * 1000, g, true)) face.say((s < 0 ? "-" : "+") + Math.abs(s) + " S", 1200);
                return;
            }
            face.request(face.transportLabels[w], "own.transport", { action: face.ownTransport[w] });
            return;
        }
        face.send(face.transportLabels[w], g.target, face.transportTags[w], {});
    }
    function seekTo(fraction, g) {
        g = face.gestureOr(g);
        const c = face.m.controls.seek;
        if (!c.enabled) { face.refuse("SEEK", c); return false; }
        const pos = Math.round(Math.max(0, Math.min(1, fraction)) * face.m.durationMs);
        if (g.mode === "own") face.request("SEEK", "own.seek", { positionMs: pos, occurrence: g.occurrence || face.m.occurrence });
        else face.send("SEEK", g.target, "transport.seek", { positionMs: pos, trackId: g.trackId || face.m.trackId });
        return true;
    }
    function seekBy(deltaMs, g, quiet) {
        const c = face.m.controls.seek;
        if (!c.enabled) { face.refuse(quiet ? (deltaMs < 0 ? "BACK" : "FORWARD") : "SEEK", c); return false; }
        return face.seekTo((face.m.positionMs + deltaMs) / face.m.durationMs, g);
    }
    // The slider is the system volume (the default audio sink) in every state;
    // it never sets a player's or a source's own volume.
    function setVolume(v) {
        const c = face.m.controls.volume;
        if (!c.enabled) { face.refuse("VOLUME", c); return; }
        v = Math.round(Math.max(0, Math.min(1, v)) * 100) / 100;
        face.request("VOLUME", "system.volume", { value: v });
        face.say("VOLUME: " + Math.round(v * 100) + "%", 1000);
    }
    function stepVolume(delta) {
        const c = face.m.controls.volume;
        if (!c.enabled) { face.refuse("VOLUME", c); return; }
        const v = View.volumeStep(View.systemVolume(face.input), delta);
        if (v === null) { face.say("VOLUME: " + Math.round(face.m.volumeReal * 100) + "%", 1000); return; }
        face.setVolume(v);
    }
    function setRepeat(mode, g) {
        g = face.gestureOr(g);
        const c = face.m.controls.repeat;
        if (!c.enabled) { face.refuse("REPEAT", c); return; }
        if (g.mode === "own") face.request("REPEAT", "own.repeat", { mode: mode });
        else face.send("REPEAT", g.target, "repeat.set", { mode: mode });
    }
    function cycleRepeat(g) {
        const c = face.m.controls.repeat;
        if (!c.enabled) { face.refuse("REPEAT", c); return; }
        const next = Keys.nextRepeat(face.m.repeat);
        face.setRepeat(next, g);
        face.say("REPEAT: " + { off: "OFF", all: "ALL", one: "ONE TRACK" }[next], 1000);
    }
    function toggleShuffle(g) {
        g = face.gestureOr(g);
        const c = face.m.controls.shuffle;
        if (!c.enabled) { face.refuse("SHUFFLE", c); return; }
        if (g.mode === "own") face.request("SHUFFLE", "own.shuffle", { enabled: !face.m.shuffle });
        else face.send("SHUFFLE", g.target, "shuffle.set", { enabled: !face.m.shuffle });
        face.say("SHUFFLE: " + (face.m.shuffle ? "OFF" : "ON"), 1000);
    }
    function setSpeed(rate) {
        const c = face.m.controls.speed;
        if (!c.enabled) { face.refuse("SPEED", c); return; }
        face.request("SPEED", "own.speed", { rate: rate });
        face.say("SPEED: " + rate.toFixed(1) + "X", 1000);
    }
    function setBalance(b) {
        const c = face.m.controls.balance;
        if (!c.enabled) { face.refuse("BALANCE", c); return; }
        face.prefs.set({ balance: b });
        face.say(View.balanceText(b), 1000);
    }

    // EQ and balance go to the engine (Pyxamp's own audio), whatever is shown.
    readonly property string eqWire: JSON.stringify(Eq.wire(face.eqShown))
    onEqWireChanged: eqSend.restart()
    readonly property real balanceWire: p.balance
    onBalanceWireChanged: balSend.restart()
    Timer { id: eqSend; interval: 80; onTriggered: face.request("EQ", "eq.set", JSON.parse(face.eqWire), { silent: false }) }
    Timer { id: balSend; interval: 80; onTriggered: face.request("BALANCE", "balance.set", { value: face.p.balance }) }
    // On connect, the engine gets the face's EQ and balance, quietly.
    function syncEngine(quiet) {
        face.request("EQ", "eq.set", JSON.parse(face.eqWire), { silent: quiet });
        face.request("BALANCE", "balance.set", { value: face.p.balance }, { silent: quiet });
    }

    // ---------- local view actions ----------
    function openPlaylist() { if (!p.plOpen) face.prefs.set({ plOpen: true }); }
    function showView(v) { face.view = face.view === v ? "queue" : v; if (face.view !== "queue") face.openPlaylist(); }
    function openJump() { face.view = "jump"; face.jump = { text: "", sel: 0, status: "idle", items: [], message: "", reqId: "" }; face.openPlaylist(); }
    function toggleDouble() {
        if (p.dbl) { face.prefs.set({ dbl: false }); return; }
        if (face.planFor(true).refused) { face.say("DOUBLE SIZE DOES NOT FIT THIS SCREEN", 2400); return; }
        face.prefs.set({ dbl: true });
    }
    function toggleTimeMode() {
        const next = p.timeMode === "elapsed" ? "remaining" : "elapsed";
        face.prefs.set({ timeMode: next });
        if (next === "remaining" && face.m.mode !== "none" && face.m.durationMs === null) face.say("NO LENGTH: TIME ELAPSED ONLY", 2000);
    }
    function setLayer(onTop) { face.prefs.set({ onTop: onTop }); face.say(onTop ? "ON TOP" : "ON THE DESKTOP", 1200); }
    function setVis(mode) { face.visChosen = true; face.prefs.set({ vis: mode }); }
    function cycleVis() { face.setVis({ spec: "osc", osc: "off", off: "spec" }[face.reducedOff ? "off" : p.vis]); }
    function setEq(changes) {
        // Moving a slider while AUTO picked a preset turns AUTO off and starts from that preset.
        if (face.eqShown.auto && (changes.bands || changes.pre !== undefined)) {
            const base = { bands: face.eqShown.bands, pre: face.eqShown.pre, auto: false };
            face.prefs.set({ eq: Object.assign({}, p.eq, base, changes) });
            face.say("EQ AUTO: OFF", 1000);
            return;
        }
        face.prefs.set({ eq: Object.assign({}, p.eq, changes) });
    }
    function applyPreset(pr, name) { face.prefs.set({ eq: Object.assign({}, p.eq, { bands: pr.bands.slice(), pre: pr.pre, auto: false }) }); face.say("EQ PRESET: " + face.upper(name), 1200); }

    function toggleAlbumMode() {
        const on = !p.albumMode;
        face.prefs.set({ albumMode: on });
        face.albumOpen = "";
        if (on && link.snap) {
            const gs = Playlist.groups(link.snap.queue);
            face.albumSel = Math.max(0, gs.findIndex((g) => g.items.some((i) => Playlist.isCurrent(face.plCtx, link.snap.queue[i]))));
        }
        if (face.view !== "queue") face.view = "queue";
        face.openPlaylist();
    }

    // SUPER+Z: show on the focused monitor at the place last left there, or hide. Instant.
    function toggle() {
        face.menu = "";
        Tips.clear();
        if (p.shown) { face.prefs.set({ shown: false }); return; }
        Hyprland.refreshMonitors();
        face.screen = face.focusedScreen();
        face.prefs.set({ shown: true });
    }
    function hide() { face.menu = ""; Tips.clear(); face.prefs.set({ shown: false }); }

    function rememberPosition(pos) {
        if (!face.screen) return;
        const all = Object.assign({}, p.positions); all[face.screen.name] = { x: pos.x, y: pos.y };
        face.prefs.set({ positions: all });
    }

    function openMenu(kind, anchor) { Tips.clear(); face.menuAnchor = anchor; face.menu = face.menu === kind ? "" : kind; }

    // ---------- the playlist section: rows ----------
    function selectable(i) { const r = face.pl.rows[i]; return !!r && !!r.ref && !r.group; }
    function setSel(i) {
        const r = face.pl.rows[i];
        if (!r || !r.ref) return;
        if (face.view === "library") face.lib = { path: face.lib.path, sel: i };
        else if (face.view === "jump") face.jump = Object.assign({}, face.jump, { sel: i });
        else if (face.view === "queue") {
            if (p.albumMode && !face.albumOpen) face.albumSel = i;
            else if (r.ref.occurrence) face.sel = r.ref.occurrence;
        }
    }
    function rowMove(delta) {
        const rows = face.pl.rows;
        let i = face.pl.sel < 0 ? (delta > 0 ? -1 : rows.length) : face.pl.sel;
        for (let k = 0; k < rows.length; k++) {
            i += delta;
            if (i < 0 || i >= rows.length) return;
            if (face.selectable(i)) { face.setSel(i); return; }
        }
    }
    // Enter, a double click, or Ins (queue) on row i.
    function rowEnter(i, queue) {
        if (face.prompt) return;
        const r = face.pl.rows[i === undefined ? face.pl.sel : i];
        if (!r || !r.ref) return;
        const ref = r.ref;
        switch (ref.type) {
        case "queue": face.jumpTo(ref); break;
        case "album": face.albumOpen = ref.first; face.sel = ref.first; break;
        case "albumBack": face.albumBack(); break;
        case "conflict": face.keepConflict(ref.conflict); break;
        case "source": face.libPush(ref); break;
        case "group": face.libPush(ref); break;
        case "item":
            if (face.view === "jump") { face.jumpPlay(ref.item, queue); break; }
            if (ref.item.openable && !queue) face.libPush({ type: "item", source: ref.item.source, node: ref.item.id, label: ref.item.title, kind: ref.item.kind });
            else face.playItem(ref.item, queue ? "last" : "now");
            break;
        case "more": face.libMore(); break;
        case "libBack": face.libBack(); break;
        case "viewBack": face.view = "queue"; break;
        }
    }
    function jumpTo(ref) {
        if (ref.conflict) { face.keepConflict(ref.conflict); return; }
        face.request("PLAY", "queue.jump", { occurrence: ref.occurrence });
    }
    function removeSelected() {
        if (!face.sel) { face.say("REMOVE: SELECT AN ITEM FIRST", 1500); return; }
        face.request("REMOVE", "queue.remove", { occurrence: face.sel }, { ok: "REMOVED FROM THE QUEUE" });
    }
    function albumBack() {
        const s = link.snap;
        if (s) { const gs = Playlist.groups(s.queue); face.albumSel = Math.max(0, gs.findIndex((g) => g.first === face.albumOpen)); }
        face.albumOpen = "";
    }
    function playItem(it, mode) {
        if (!it.playable) { face.say(face.upper(it.title) + ": NOTHING TO PLAY", 1500); return; }
        const from = Playlist.activeSource(face.plCtx), name = View.sourceLabel(it.source);
        let ok = mode === "last" ? "QUEUED IN " + face.upper(name) : mode === "next" ? "PLAYS NEXT IN " + face.upper(name)
            : it.kind === "video" ? face.upper(it.title) + ": OPENS IN MPV"
            : from && from !== it.source ? face.upper(View.sourceLabel(from)) + " PAUSED. PLAYING FROM " + face.upper(name) : "PLAYING: " + face.upper(it.title);
        face.request(mode === "last" ? "QUEUE" : "PLAY", "item.play", { source: it.source, item: it.id, mode: mode }, { ok: ok, okMs: 2200 });
    }
    function startStation(it) {
        face.request("STATION", "station.start", { source: it.source, seed: it.id }, { ok: "STATION FROM " + face.upper(it.kind === "album" && it.subtitle ? it.subtitle : it.title) + " (" + face.upper(it.kind) + ")", okMs: 2200 });
    }
    function markPlayed(it, played) {
        face.request("MARK", "item.markPlayed", { source: it.source, item: it.id, played: played }, { ok: played ? "MARKED AS PLAYED (SYNCED)" : "MARKED AS UNPLAYED (SYNCED)", okMs: 1500 });
    }
    function addToPlaylist(pl, it) {
        face.request("ADD TO PLAYLIST", "playlist.add", { source: it.source, playlist: pl.id, item: it.id }, { ok: "ADDED TO PLAYLIST: " + face.upper(pl.title), okMs: 1800 });
    }
    function selectOutput(o) {
        face.request("PLAY ON", "output.select", { source: face.m.source, output: o.id }, { ok: o.local ? "PLAYS HERE" : "PLAYS ON " + face.upper(o.name), okMs: 1500 });
    }

    // ---------- position conflict (decision 36) ----------
    readonly property var conflict: link.snap && link.snap.conflicts.length ? link.snap.conflicts[0] : null
    function pickConflict(delta) {
        if (!face.conflict) return;
        const n = face.conflict.positions.length;
        face.conflictPick = ((face.conflictPick + delta) % n + n) % n;
    }
    function keepConflict(c, pick) {
        c = c || face.conflict;
        if (!c) return;
        const k = pick === undefined ? Math.min(face.conflictPick, c.positions.length - 1) : pick, pos = c.positions[k];
        face.request("KEEP", "conflict.resolve", { source: c.item.source, conflict: c.id, keep: pos.id },
                     { ok: "KEPT " + face.upper(pos.device) + " " + View.formatSeconds(pos.positionMs / 1000) + " (SYNCED)", okMs: 2000 });
    }

    // ---------- library view ----------
    function libKey(e) { return Playlist.pageKey(e.source, e.node); }
    function libLoad(e, cursor) {
        const key = face.libKey(e), pg = face.libPages[key];
        if (cursor === undefined && pg && (pg.status === "ready" || pg.status === "loading")) return;
        const payload = { source: e.source, node: e.node };
        if (cursor !== undefined) payload.cursor = cursor;
        if (cursor === undefined) face.setPage(key, { status: "loading", items: [] });
        else face.setPage(key, Object.assign({}, pg, { loadingMore: true }));
        face.request("LIBRARY", "library.open", payload, { lib: { key: key, append: cursor !== undefined }, silent: false });
    }
    function libPush(ref) {
        if (ref.type === "source") {
            const s = Playlist.summary(face.plCtx, ref.source);
            if (s && s.status._tag === "unavailable") { face.say(face.upper(s.label) + " UNREACHABLE: " + s.status.message, 2500); return; }
            if (s && s.status._tag === "connecting") { face.say(face.upper(s.label) + " IS STILL CONNECTING", 1500); return; }
        }
        const path = face.lib.path.map((e) => e);
        if (path.length) path[path.length - 1] = Object.assign({}, path[path.length - 1], { sel: face.pl.sel });
        else face.libRootSel = face.pl.sel;
        const e = { source: ref.source, label: ref.label, kind: ref.kind };
        if (ref.node !== undefined) e.node = ref.node;
        path.push(e);
        face.lib = { path: path, sel: 0 };
        if (e.node !== undefined) face.libLoad(e);
    }
    property int libRootSel: 0
    function libBack() {
        const path = face.lib.path.slice();
        if (!path.length) { face.view = "queue"; return; }
        path.pop();
        face.lib = { path: path, sel: path.length ? (path[path.length - 1].sel || 0) : face.libRootSel };
    }
    function libMore() {
        const path = face.lib.path, e = path[path.length - 1];
        if (!e) return;
        const pg = face.libPages[face.libKey(e)];
        if (pg && pg.nextCursor !== undefined && !pg.loadingMore) face.libLoad(e, pg.nextCursor);
    }
    function libRefresh() {
        const path = face.lib.path, e = path[path.length - 1];
        if (e && e.node !== undefined) { const next = Object.assign({}, face.libPages); delete next[face.libKey(e)]; face.libPages = next; face.libLoad(e); }
    }
    // Pyxis playlists for "Add to playlist": the source's playlists group, as reported.
    readonly property var pyxisPlaylistsGroup: {
        const s = Playlist.summary(face.plCtx, "pyxis");
        if (!s) return null;
        for (const g of s.groups) if (g.kind === "playlists") return { source: "pyxis", node: g.id, label: g.label, kind: g.kind };
        return null;
    }

    // ---------- jump box ----------
    function jumpType(text) {
        if (face.jump.text.length >= 80) return;
        face.jump = Object.assign({}, face.jump, { text: face.jump.text + text, sel: 0 });
        searchSoon.restart();
    }
    function jumpBackspace() {
        face.jump = Object.assign({}, face.jump, { text: face.jump.text.slice(0, -1), sel: 0 });
        searchSoon.restart();
    }
    Timer {
        id: searchSoon
        interval: 250
        onTriggered: {
            const q = face.jump.text.trim();
            if (!q) { face.jump = Object.assign({}, face.jump, { status: "idle", items: [], reqId: "" }); return; }
            const id = face.request("SEARCH", "search", { query: q.slice(0, 200) }, { jump: true, silent: true });
            face.jump = Object.assign({}, face.jump, { status: id ? "searching" : "error", message: id ? "" : "the daemon is not connected", reqId: id });
        }
    }
    function jumpPlay(it, queue) {
        if (!it.playable && it.openable) {
            face.view = "library";
            face.lib = { path: [], sel: 0 };
            const s = Playlist.summary(face.plCtx, it.source);
            face.libPush({ type: "source", source: it.source, label: s ? s.label : View.sourceLabel(it.source) });
            face.libPush({ type: "item", source: it.source, node: it.id, label: it.title, kind: it.kind });
            return;
        }
        face.playItem(it, queue ? "last" : "now");
        face.view = "queue";
    }

    // ---------- the prompt line (decision 32) ----------
    function openPrompt(kind) {
        face.menu = "";
        if (kind === "savepl") {
            if (face.m.source !== "pyxis" || !link.snap || !link.snap.queue.length) { face.say("SAVE AS PLAYLIST: NOTHING QUEUED IN PYXIS", 2000); return; }
            face.prompt = { kind: kind, title: "SAVE AS PLAYLIST", label: "Name", text: "", right: link.snap.queue.length + " tracks", hint: "Enter: save in Pyxis   Esc: cancel", refold: !p.plOpen };
        } else {
            face.prompt = { kind: kind, title: "SAVE EQ PRESET", label: "Name", text: "", right: "10 bands + preamp", hint: "Enter: save   Esc: cancel", refold: !p.plOpen };
        }
        face.openPlaylist();
    }
    function closePrompt(save) {
        const P = face.prompt;
        if (!P) return;
        face.prompt = null;
        if (P.refold) face.prefs.set({ plOpen: false });
        if (!save) return;
        const name = P.text.trim();
        if (!name) { face.say("NO NAME: NOTHING SAVED", 1500); return; }
        if (P.kind === "savepl") face.request("SAVE AS PLAYLIST", "playlist.save", { source: "pyxis", name: name }, { ok: "SAVED AS PLAYLIST: " + face.upper(name), okMs: 3500 });
        else {
            const list = p.eqPresets.filter((x) => x.name !== name).concat([{ name: name, pre: face.eqShown.pre, bands: face.eqShown.bands.slice() }]).slice(-Eq.USER_PRESETS_MAX);
            face.prefs.set({ eqPresets: list });
            face.say("EQ PRESET SAVED: " + face.upper(name), 2000);
        }
    }
    function promptType(text) { if (face.prompt && face.prompt.text.length < Eq.NAME_MAX) face.prompt = Object.assign({}, face.prompt, { text: face.prompt.text + text }); }

    // ---------- keys: lib/keys.js decides; this runs the action ----------
    function key(event) {
        const a = Keys.decide(Keys.fromQt(event.key, event.text, event.modifiers),
                              { view: face.view, plOpen: p.plOpen, prompt: !!face.prompt, albumMode: p.albumMode, albumOpen: !!face.albumOpen, conflict: face.m.conflict });
        if (!a) return false;
        switch (a._tag) {
        case "transport": face.transport(a.which); break;
        case "double": face.toggleDouble(); break;
        case "timeMode": face.toggleTimeMode(); break;
        case "shade": face.prefs.set({ shade: !p.shade }); break;
        case "foldEq": face.prefs.set({ eqOpen: !p.eqOpen }); break;
        case "foldPl": face.prefs.set({ plOpen: !p.plOpen }); break;
        case "info": face.showView("info"); break;
        case "library": face.showView("library"); break;
        case "albumMode": face.toggleAlbumMode(); break;
        case "jump": face.openJump(); break;
        case "jumpType": face.jumpType(a.text); break;
        case "jumpBackspace": face.jumpBackspace(); break;
        case "jumpClose": face.view = "queue"; face.jump = Object.assign({}, face.jump, { text: "", items: [], status: "idle" }); break;
        case "jumpMove": face.rowMove(a.delta); break;
        case "jumpEnter": face.rowEnter(face.pl.sel, a.queue); break;
        case "promptType": face.promptType(a.text); break;
        case "promptBackspace": face.prompt = Object.assign({}, face.prompt, { text: face.prompt.text.slice(0, -1) }); break;
        case "promptCancel": face.closePrompt(false); break;
        case "promptEnter": face.closePrompt(true); break;
        case "conflictPick": face.pickConflict(a.delta); break;
        case "conflictKeep": face.keepConflict(); break;
        case "rowMove": face.rowMove(a.delta); break;
        case "rowEnter": face.rowEnter(face.pl.sel, a.queue); break;
        case "rowRemove": face.removeSelected(); break;
        case "albumBack": face.albumBack(); break;
        case "libBack": face.libBack(); break;
        case "viewBack": face.view = "queue"; break;
        case "repeat": face.cycleRepeat(); break;
        case "shuffle": face.toggleShuffle(); break;
        case "volume": face.stepVolume(a.delta); break;
        case "seek": face.seekBy(a.deltaMs); break;
        }
        return true;
    }

    // A right click on row i of the playlist section, at anchor.
    function openRowMenu(i, anchor) {
        if (face.prompt) return;
        const r = face.pl.rows[i];
        if (!r || !r.ref) return;
        face.setSel(i);
        const ref = r.ref;
        let kind = "";
        if (ref.type === "queue" || ref.type === "conflict") kind = "row";
        else if (ref.type === "item") kind = face.view === "jump" ? "jumprow" : "librow";
        else if (ref.type === "album") kind = "albumrow";
        if (!kind) return;
        face.menuRow = ref;
        if (ref.item && ref.item.source === "pyxis" && face.pyxisPlaylistsGroup) face.libLoad(face.pyxisPlaylistsGroup);
        face.openMenu(kind, anchor);
    }

    // ---------- menus (Win9x style popups outside the surface) ----------
    function playlistItems() {
        const g = face.pyxisPlaylistsGroup, it = face.menuRow && face.menuRow.item;
        if (!g || !it) return [{ l: "Pyxis reports no playlists", dis: true }];
        const pg = face.libPages[face.libKey(g)];
        if (!pg || pg.status === "loading") return [{ l: "Loading playlists...", dis: true }];
        if (pg.status === "error") return [{ l: "Pyxis did not answer", dis: true }];
        const lists = pg.items.filter((x) => x.kind === "playlist");
        if (!lists.length) return [{ l: "No playlists yet", dis: true }];
        return lists.map((pl) => ({ l: pl.title, act: () => face.addToPlaylist(pl, it) }));
    }
    function menuItems(kind) {
        const c = face.m.controls, noLength = face.m.durationMs === null, outs = link.snap ? link.snap.outputs : [];
        const sw = face.m.spoken, row = face.menuRow, it = row && row.item;
        switch (kind) {
        case "main": return [
            { l: "Play on", sub: "playon", dis: !outs.length, key: outs.length ? "" : "Pyxis only" },
            { sep: true },
            { l: "Time elapsed", radio: p.timeMode === "elapsed", act: () => face.prefs.set({ timeMode: "elapsed" }), key: "Ctrl+T" },
            { l: "Time remaining", radio: p.timeMode === "remaining", act: () => face.prefs.set({ timeMode: "remaining" }), dis: noLength },
            { sep: true },
            { l: "Repeat", sub: "repeat", dis: !c.repeat.enabled },
            { l: "Shuffle", check: face.m.shuffle === true, act: () => face.toggleShuffle(), key: "S", dis: !c.shuffle.enabled },
            { sep: true },
            { l: "On top", radio: p.onTop, act: () => face.setLayer(true), key: "A" },
            { l: "On the desktop", radio: !p.onTop, act: () => face.setLayer(false) },
            { l: "Double size", check: p.dbl, act: () => face.toggleDouble(), key: "Ctrl+D", dis: !p.dbl && face.planFor(true).refused },
            { l: "Windowshade", check: p.shade, act: () => face.prefs.set({ shade: !p.shade }), key: "Alt+W" },
            { sep: true },
            { l: "Equalizer", check: p.eqOpen, act: () => face.prefs.set({ eqOpen: !p.eqOpen }), key: "Alt+G" },
            { l: "Playlist", check: p.plOpen, act: () => face.prefs.set({ plOpen: !p.plOpen }), key: "Alt+E" },
            { l: "Library", act: () => face.showView("library"), key: "L" },
            { l: "Jump to item...", act: () => face.openJump(), key: "J" },
            { sep: true },
            { l: "Reduced motion", check: p.reducedMotion, act: () => { face.visChosen = false; face.prefs.set({ reducedMotion: !p.reducedMotion }); } },
            { sep: true },
            { l: "Hide Pyxamp", act: () => face.hide(), key: "Super+Z" },
        ];
        case "playon": return outs.map((o) => ({ l: o.name, radio: o.active, key: o.local ? "here" : "", act: () => face.selectOutput(o) }));
        case "repeat": return [
            { l: "Off", radio: face.m.repeat === "off", act: () => face.setRepeat("off") },
            { l: "All", radio: face.m.repeat === "all", act: () => face.setRepeat("all") },
            { l: "One track", radio: face.m.repeat === "one", act: () => face.setRepeat("one") },
        ];
        case "vis": {
            const mode = face.reducedOff ? "" : p.vis;
            return [
                { l: "Spectrum analyzer", radio: mode === "spec", act: () => face.setVis("spec") },
                { l: "Oscilloscope", radio: mode === "osc", act: () => face.setVis("osc") },
                { l: "Off", radio: mode === "off", act: () => face.setVis("off") },
                { sep: true },
                { l: "MilkDrop (later)", dis: true },
            ];
        }
        case "presets": return Eq.PRESETS.map((pr) => ({ l: pr.label, key: pr.id === "voice" ? "auto: spoken word" : "", act: () => face.applyPreset(pr, pr.label) }))
            .concat(p.eqPresets.length ? [{ sep: true }] : [])
            .concat(p.eqPresets.map((u) => ({ l: u.name, act: () => face.applyPreset(u, u.name) })))
            .concat([{ sep: true }, { l: "Save as new preset...", act: () => face.openPrompt("eq") }, { l: "Delete preset", sub: "eqdel", dis: !p.eqPresets.length }]);
        case "eqdel": return p.eqPresets.map((u) => ({ l: u.name, act: () => { face.prefs.set({ eqPresets: p.eqPresets.filter((x) => x.name !== u.name) }); face.say("EQ PRESET DELETED: " + face.upper(u.name), 1500); } }));
        case "add": return [
            { l: "From the library", act: () => face.showView("library"), key: "L" },
            { l: "With the jump box", act: () => face.openJump(), key: "J" },
        ];
        case "rem": return [
            { l: "Remove selected", act: () => face.removeSelected(), key: "Del", dis: !face.sel },
            { l: "Crop selection", dis: true },
            { l: "Clear the queue", dis: true, key: "Ctrl+N" },
        ];
        case "sel": return [
            { l: "Select all", dis: true, key: "Ctrl+A" },
            { l: "Select none", act: () => { face.sel = ""; }, dis: !face.sel },
            { l: "Invert selection", dis: true, key: "Ctrl+I" },
        ];
        case "misc": {
            const cur = face.currentQueueItem();
            return [
                { l: "Item info", act: () => face.showView("info"), key: "Alt+3" },
                { l: "Mark as played", act: () => face.markPlayed(cur.item, true), dis: !sw || !cur },
                { sep: true },
                { l: "Album mode", check: p.albumMode, act: () => face.toggleAlbumMode(), key: "G" },
                { l: "Save queue as playlist...", act: () => face.openPrompt("savepl"), dis: face.m.source !== "pyxis" },
                { sep: true },
                { l: "Sort by title", dis: true },
                { l: "Sort by artist", dis: true },
                { l: "Reverse list", dis: true },
            ];
        }
        case "row": {
            if (!row) return [];
            if (row.type === "conflict" || row.conflict) {
                const cf = row.conflict;
                return cf.positions.map((x, k) => ({ l: "Keep " + x.device + " " + View.formatSeconds(x.positionMs / 1000), act: () => face.keepConflict(cf, k), bold: k === face.conflictPick }));
            }
            const play = { l: "Play", act: () => face.jumpTo(row), bold: true };
            const info = { l: "Item info", act: () => face.showView("info"), key: "Alt+3" };
            const rem = { l: it.source === "ossicle" ? "Remove from up next" : "Remove", act: () => { face.sel = row.occurrence; face.removeSelected(); }, key: "Del" };
            if (it.source === "ossicle") {
                const played = !!(it.progress && it.progress.played);
                return [play, { l: "Mark as played", act: () => face.markPlayed(it, true), dis: played }, { l: "Mark as unplayed", act: () => face.markPlayed(it, false), dis: !played }, { sep: true }, info, rem];
            }
            if (it.source === "pyxis") return [play, { sep: true }, { l: "Add to playlist", sub: "addpl" }, { l: "Start station", act: () => face.startStation(it) }, { sep: true }, info, rem];
            return [play, { sep: true }, info, rem];
        }
        case "albumrow": return [
            { l: "Open", act: () => { face.albumOpen = row.first; face.sel = row.first; }, bold: true, key: "Enter" },
            { l: "Play from here", act: () => face.request("PLAY", "queue.jump", { occurrence: row.first }) },
        ];
        case "librow": case "jumprow": {
            if (!it) return [];
            const out = [];
            if (it.openable && kind === "librow") out.push({ l: "Open", act: () => face.libPush({ type: "item", source: it.source, node: it.id, label: it.title, kind: it.kind }), bold: !it.playable });
            if (it.playable) out.push({ l: it.kind === "video" ? "Open in mpv" : "Play", act: () => kind === "jumprow" ? face.jumpPlay(it, false) : face.playItem(it, "now"), bold: true },
                                      { l: "Play next", act: () => face.playItem(it, "next"), dis: it.kind === "video" },
                                      { l: "Queue", act: () => face.playItem(it, "last"), key: "Ins", dis: it.kind === "video" });
            if (it.source === "pyxis" && it.kind === "track") out.push({ sep: true }, { l: "Add to playlist", sub: "addpl" });
            if (it.source === "pyxis" && (it.kind === "track" || it.kind === "artist" || it.kind === "album"))
                out.push({ sep: true }, { l: it.kind === "album" && it.subtitle ? "Start station from " + it.subtitle : "Start station", act: () => face.startStation(it) });
            if (it.source === "ossicle" && (it.kind === "episode" || it.kind === "book")) {
                const played = !!(it.progress && it.progress.played);
                out.push({ sep: true }, { l: played ? "Mark as unplayed" : "Mark as played", act: () => face.markPlayed(it, !played) });
            }
            return out.length ? out : [{ l: "Nothing to do here", dis: true }];
        }
        case "addpl": return face.playlistItems();
        }
        return [];
    }
    function currentQueueItem() {
        const s = link.snap;
        if (!s || !s.own) return null;
        for (const q of s.queue) if (q.occurrence === face.sel) return q;
        for (const q of s.queue) if (q.occurrence === s.own.occurrence) return q;
        return null;
    }

    // Where the surface goes when Pyxamp starts: the focused monitor.
    Component.onCompleted: face.screen = face.focusedScreen()
    Connections {
        target: Quickshell
        function onScreensChanged() { if (!face.screen || Quickshell.screens.indexOf(face.screen) < 0) face.screen = face.focusedScreen(); }
    }

    PyxampPanel { face: face }
}
