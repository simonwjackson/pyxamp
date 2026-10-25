import QtQuick
import Quickshell
import Quickshell.Hyprland
import "lib/view.js" as View
import "lib/fit.js" as Fit
import "lib/vis.js" as Vis
import "lib/keys.js" as Keys
import "lib/protocol.js" as Protocol

// The composition root of the face: it reads the daemon link and the local
// preferences, derives the view model once (lib/view.js), owns the local view
// state and every action, and hands one object to the sections below. Nothing
// below this file reads the link or the preferences directly.
Scope {
    id: face
    required property DaemonLink link
    required property Prefs prefs
    required property MonoClock clock

    // ---------- local view state ----------
    property string view: "queue"           // queue | library | jump | info (the playlist section)
    property string jumpText: ""
    property var flash: null                // { text, t0, until }
    property real seekDrag: -1              // 0..1 while the position thumb is held
    property real volumeDrag: -1            // 0..1 while the volume knob is held
    property bool visChosen: false          // reduced motion: a vis mode was picked this session
    property string menu: ""                // open popup menu
    property Item menuAnchor: null
    property var gestureTarget: null        // the followed player when a gesture began
    property string gestureTrack: ""
    property var pending: ({})              // command id -> label
    property real now: clock.ms
    property ShellScreen screen: null       // where the surface is (set on show)
    property var dragPos: null              // live position while the title bar is dragged

    readonly property var p: prefs.value
    readonly property bool reducedOff: p.reducedMotion && !face.visChosen
    readonly property var input: ({
        link: link.link, player: link.player, observedAt: link.observedAt, now: face.now,
        timeMode: p.timeMode, flash: face.flash, seekDrag: face.seekDrag >= 0 ? face.seekDrag : null,
        marqueeSince: face.marqueeSince,
    })
    // The marquee restarts its scroll only when its text changes.
    property real marqueeSince: 0
    readonly property string marqueeBase: View.marquee({ link: link.link, player: link.player, observedAt: 0, now: 0, flash: null }).text
    onMarqueeBaseChanged: face.marqueeSince = face.clock.now()
    readonly property var m: View.main(face.input)
    readonly property var visual: View.visual(face.input, link.frame, p.vis, face.reducedOff)
    readonly property var info: View.itemInfo(face.input)

    // ---------- monitor, scale, fit ----------
    function focusedScreen() {
        const mon = Hyprland.focusedMonitor;
        if (mon) for (const s of Quickshell.screens) if (s.name === mon.name) return s;
        return Quickshell.screens.length ? Quickshell.screens[0] : null;
    }
    readonly property real monitorScale: face.screen ? face.screen.devicePixelRatio : 1
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
    Binding { target: face.clock; property: "animating"; value: face.p.shown && (face.visMoving || face.m.marquee.text.length > View.MARQUEE_CHARS || face.m.status === "play") }

    // ---------- feedback ----------
    function say(text, ms) { face.flash = { text: text, t0: face.clock.now(), until: face.clock.now() + (ms || 1600) }; }
    function refuse(label, control) { face.say(label + ": " + control.reason, 2400); }

    Connections {
        target: face.link
        function onOutcome(id, result) {
            const label = face.pending[id];
            if (label === undefined) return;
            const next = Object.assign({}, face.pending); delete next[id]; face.pending = next;
            if (result._tag === "rejected") face.say(label + " REFUSED: " + result.message, 3000);
        }
        function onLinkChanged() { if (face.link.link._tag !== "connected") { face.pending = ({}); face.seekDrag = -1; face.volumeDrag = -1; } }
    }

    // ---------- commands to the followed player ----------
    // A gesture keeps the target it began with (contract: commands carry the
    // target shown when the gesture began).
    function beginGesture() {
        face.gestureTarget = face.m.target;
        face.gestureTrack = face.m.target && face.link.player.track.id !== undefined ? face.link.player.track.id : "";
    }
    function send(label, target, tag, payload) {
        if (!target) return;
        const id = face.link.command(target, Protocol.action(tag, payload));
        if (id) { const next = Object.assign({}, face.pending); next[id] = label; face.pending = next; }
    }
    readonly property var transportTags: ({ prev: "transport.previous", play: "transport.play", pause: "transport.pause", stop: "transport.stop", next: "transport.next" })
    readonly property var transportLabels: ({ prev: "PREVIOUS", play: "PLAY", pause: "PAUSE", stop: "STOP", next: "NEXT" })

    // which: prev | play | pause | stop | next. target: from the gesture, or now.
    function transport(which, target) {
        let w = which;
        // C and the pause button toggle, as in Winamp: paused -> play.
        if (w === "pause" && face.link.player && face.link.player._tag === "following" && face.link.player.transport === "paused") w = "play";
        const c = face.m.controls[w];
        if (!c.enabled) { face.refuse(face.transportLabels[w], c); return; }
        face.send(face.transportLabels[w], target || face.m.target, face.transportTags[w], {});
    }
    function seekTo(fraction, target, trackId) {
        const c = face.m.controls.seek;
        if (!c.enabled) { face.refuse("SEEK", c); return; }
        const d = face.link.player.track.durationMs, pos = Math.round(Math.max(0, Math.min(1, fraction)) * d);
        face.send("SEEK", target || face.m.target, "transport.seek", { positionMs: pos, trackId: trackId || face.link.player.track.id });
    }
    function seekBy(deltaMs) {
        const c = face.m.controls.seek;
        if (!c.enabled) { face.refuse("SEEK", c); return; }
        const d = face.link.player.track.durationMs;
        face.seekTo((face.m.positionMs + deltaMs) / d);
    }
    function setVolume(v, target) {
        const c = face.m.controls.volume;
        if (!c.enabled) { face.refuse("VOLUME", c); return; }
        v = Math.round(Math.max(0, Math.min(1, v)) * 100) / 100;
        face.send("VOLUME", target || face.m.target, "volume.set", { value: v });
        face.say("VOLUME: " + Math.round(v * 100) + "%", 1000);
    }
    function cycleRepeat(target) {
        const c = face.m.controls.repeat;
        if (!c.enabled) { face.refuse("REPEAT", c); return; }
        const next = Keys.nextRepeat(face.m.repeat);
        face.send("REPEAT", target || face.m.target, "repeat.set", { mode: next });
        face.say("REPEAT: " + { off: "OFF", all: "ALL", one: "ONE TRACK" }[next], 1000);
    }
    function setRepeat(mode) {
        const c = face.m.controls.repeat;
        if (!c.enabled) { face.refuse("REPEAT", c); return; }
        face.send("REPEAT", face.m.target, "repeat.set", { mode: mode });
    }
    function toggleShuffle(target) {
        const c = face.m.controls.shuffle;
        if (!c.enabled) { face.refuse("SHUFFLE", c); return; }
        face.send("SHUFFLE", target || face.m.target, "shuffle.set", { enabled: !face.m.shuffle });
        face.say("SHUFFLE: " + (face.m.shuffle ? "OFF" : "ON"), 1000);
    }

    // ---------- local view actions ----------
    function openPlaylist() { if (!p.plOpen) face.prefs.set({ plOpen: true }); }
    function showView(v) { face.view = face.view === v ? "queue" : v; if (face.view !== "queue") face.openPlaylist(); }
    function openJump() { face.view = "jump"; face.jumpText = ""; face.openPlaylist(); }
    function toggleDouble() {
        if (p.dbl) { face.prefs.set({ dbl: false }); return; }
        if (face.planFor(true).refused) { face.say("DOUBLE SIZE DOES NOT FIT THIS SCREEN", 2400); return; }
        face.prefs.set({ dbl: true });
    }
    function toggleTimeMode() {
        const next = p.timeMode === "elapsed" ? "remaining" : "elapsed";
        face.prefs.set({ timeMode: next });
        if (next === "remaining" && face.m.target && face.link.player.track.durationMs === undefined) face.say("NO LENGTH: TIME ELAPSED ONLY", 2000);
    }
    function setLayer(onTop) { face.prefs.set({ onTop: onTop }); face.say(onTop ? "ON TOP" : "ON THE DESKTOP", 1200); }
    function setVis(mode) { face.visChosen = true; face.prefs.set({ vis: mode }); }
    function cycleVis() { face.setVis({ spec: "osc", osc: "off", off: "spec" }[face.reducedOff ? "off" : p.vis]); }
    function setEq(changes) { face.prefs.set({ eq: Object.assign({}, p.eq, changes) }); }

    // SUPER+Z: show on the focused monitor at the place last left there, or hide. Instant.
    function toggle() {
        face.menu = "";
        Tips.clear();
        if (p.shown) { face.prefs.set({ shown: false }); return; }
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

    // Keys: lib/keys.js decides; this runs the action.
    function key(event) {
        const a = Keys.decide(Keys.fromQt(event.key, event.text, event.modifiers), { view: face.view, plOpen: p.plOpen });
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
        case "jump": face.openJump(); break;
        case "jumpType": if (face.jumpText.length < 80) face.jumpText += a.text; break;
        case "jumpBackspace": face.jumpText = face.jumpText.slice(0, -1); break;
        case "jumpClose": face.view = "queue"; face.jumpText = ""; break;
        case "jumpEnter": break; // no sources, so no results to play or queue
        case "viewBack": face.view = "queue"; break;
        case "repeat": face.cycleRepeat(); break;
        case "shuffle": face.toggleShuffle(); break;
        case "volume": face.setVolume((face.m.volume === null ? 0 : face.m.volume) + a.delta); break;
        case "seek": face.seekBy(a.deltaMs); break;
        }
        return true;
    }

    // ---------- menus (Win9x style popups outside the surface) ----------
    function menuItems(kind) {
        const c = face.m.controls, noLength = !(face.m.target && face.link.player.track.durationMs !== undefined);
        switch (kind) {
        case "main": return [
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
        case "presets": return [
            { l: "Flat", act: () => face.setEq({ bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], pre: 0 }) },
            { l: "Voice", act: () => face.setEq({ bands: [-6, -4, -1, 2, 4, 4, 3, 1, -2, -4] }) },
            { l: "Small speakers", act: () => face.setEq({ bands: [6, 5, 3, 1, 0, 0, 1, 2, 3, 3] }) },
            { l: "Night", act: () => face.setEq({ bands: [-3, -2, 0, 1, 1, 0, -1, -3, -5, -6] }) },
            { sep: true },
            { l: "Save as new preset...", dis: true, key: "later" },
        ];
        case "add": return [
            { l: "From the library", act: () => face.showView("library"), key: "L" },
            { l: "With the jump box", act: () => face.openJump(), key: "J" },
        ];
        case "rem": return [
            { l: "Remove selected", dis: true, key: "Del" },
            { l: "Crop selection", dis: true },
            { l: "Clear the queue", dis: true, key: "Ctrl+N" },
        ];
        case "sel": return [
            { l: "Select all", dis: true, key: "Ctrl+A" },
            { l: "Select none", dis: true },
            { l: "Invert selection", dis: true, key: "Ctrl+I" },
        ];
        case "misc": return [
            { l: "Item info", act: () => face.showView("info"), key: "Alt+3" },
            { sep: true },
            { l: "Sort by title", dis: true },
            { l: "Sort by artist", dis: true },
            { l: "Reverse list", dis: true },
        ];
        }
        return [];
    }

    // Where the surface goes when Pyxamp starts: the focused monitor.
    Component.onCompleted: face.screen = face.focusedScreen()
    Connections {
        target: Quickshell
        function onScreensChanged() { if (!face.screen || Quickshell.screens.indexOf(face.screen) < 0) face.screen = face.focusedScreen(); }
    }

    PyxampPanel { face: face }
}
