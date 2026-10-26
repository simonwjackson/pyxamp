import QtQuick
import "lib/platinum.js" as Pt
import "lib/lcdtext.js" as Lcd
import "lib/lcdcase.js" as Case
import "lib/sliders.js" as Sliders
import "lib/view.js" as View
import "lib/vis.js" as Vis

// The Platinum face's main section (300 x 150 sp): the windoid title bar, the
// clutterbar, the LCD behind its sunken bezel, the seek slider, the transport,
// volume and balance (speed for spoken word), and the sticky toggles.
// Geometry from the approved mock (/tmp/macamp-mock/mac.js drawMacMain). Every
// control calls the same Face.qml action as the Winamp face's.
Item {
    id: root
    required property var face
    property bool active: true
    readonly property var pa: face.pal
    readonly property var lc: face.lcdSet
    readonly property var m: face.m
    readonly property var c: m.controls
    width: 300
    height: 150

    Rectangle { width: 300; height: 150; color: root.pa.face }
    PlatinumRects {
        rects: { const o = []; if (!root.pa.flat) { Pt.R(o, 1, 18, 298, 1, root.pa.hi); Pt.R(o, 1, 18, 1, 132, root.pa.hi); Pt.R(o, 298, 19, 1, 131, root.pa.lo); } return o; }
    }
    PlatinumTitle { face: root.face; active: root.active }

    // ---------- clutterbar: O A I D V ----------
    // The model is fixed: a delegate rebuilt while its menu opens would take the
    // menu's anchor with it, so "on" and the tip are worked out in the delegate.
    Repeater {
        model: ["O", "A", "I", "D", "V"]
        PlatinumButton {
            id: cl
            required property string modelData
            required property int index
            pal: root.pa
            x: 7; y: 24 + index * 11; width: 14; height: 11
            on: ({ O: root.face.menu === "main", A: root.face.p.onTop, I: root.face.view === "info" && root.face.p.plOpen, D: root.face.p.dbl, V: root.face.menu === "vis" })[modelData]
            tip: ({
                O: "Options", A: root.face.p.onTop ? "On top. Click: on the desktop" : "On the desktop. Click: on top", I: "Item info (Alt+3)",
                D: !root.face.p.dbl && root.face.planFor(true).refused ? "Double size does not fit this screen" : "Double size (Ctrl+D)", V: "Visualizer",
            })[modelData]
            onActivated: {
                switch (modelData) {
                case "O": root.face.openMenu("main", cl); break;
                case "A": root.face.setLayer(!root.face.p.onTop); break;
                case "I": root.face.showView("info"); break;
                case "D": root.face.toggleDouble(); break;
                case "V": root.face.openMenu("vis", cl); break;
                }
            }
            PlatinumText { face: root.face; size: 10; text: cl.modelData; color: root.pa.text; ty: 1 + cl.o; x: 7 + cl.o - Math.round(implicitWidth / 2) }
        }
    }

    // ---------- the display: a sunken bezel, black glass edge, the LCD ----------
    PlatinumRects {
        rects: {
            const o = [];
            Pt.sunken(o, root.pa, 24, 23, 269, 57, "");
            Pt.R(o, 25, 24, 267, 55, root.pa.flat ? root.pa.edge : "#000000");
            Pt.R(o, 26, 25, 265, 53, root.lc.bg);
            Pt.R(o, 114, 28, 1, 47, root.lc.ghost);                    // divider
            return o;
        }
    }
    // Status glyph: play, pause, stop (error: in the error ink), a ghost play when idle.
    PlatinumRects {
        readonly property string st: root.m.status
        rects: {
            const o = [], ink = st === "error" ? root.lc.err : root.lc.ink;
            if (st === "play") Pt.glyph(o, "play", 31, 31, ink);
            else if (st === "pause") Pt.glyph(o, "pause", 31, 31, ink);
            else if (st === "stop" || st === "error") Pt.glyph(o, "square", 31, 32, ink);
            else Pt.glyph(o, "play", 31, 31, root.lc.ghost);
            return o;
        }
    }
    // Clock: seven segments with ghosts; blinks while own playback is paused.
    property bool blinkOff: false
    readonly property bool blinking: root.m.status === "pause" && root.m.mode === "own" && root.m.clock !== null && !root.face.p.reducedMotion
    onBlinkingChanged: root.blinkOff = false
    Timer { interval: 500; repeat: true; running: root.blinking && root.face.p.shown; onTriggered: root.blinkOff = !root.blinkOff }
    PlatinumRects {
        readonly property var ck: root.m.clock && !(root.blinking && root.blinkOff) ? root.m.clock : null
        rects: {
            const o = [], L = root.lc, on = ck !== null, x0 = 42, y0 = 29;
            Pt.R(o, x0, y0 + 7, 5, 2, on && ck.minus ? L.ink : L.ghost);
            const d = on ? ck.digits : [null, null, null, null];
            Pt.seg7(o, d[0], x0 + 8, y0, L.ink, L.ghost); Pt.seg7(o, d[1], x0 + 19, y0, L.ink, L.ghost);
            Pt.R(o, x0 + 30, y0 + 4, 2, 2, on ? L.ink : L.ghost); Pt.R(o, x0 + 30, y0 + 9, 2, 2, on ? L.ink : L.ghost);
            Pt.seg7(o, d[2], x0 + 35, y0, L.ink, L.ghost); Pt.seg7(o, d[3], x0 + 46, y0, L.ink, L.ghost);
            return o;
        }
    }
    PlatinumLcdText { x: 99; y: 33; text: "h"; ink: root.lc.ink; visible: !!root.m.clock && root.m.clock.hours && !(root.blinking && root.blinkOff) }
    Hotspot { x: 40; y: 27; width: 70; height: 19; tip: "Elapsed or remaining (Ctrl+T)"; onActivated: root.face.toggleTimeMode() }

    // Visualizer: 19 bars of 7 segments, peaks in the mid tone.
    PlatinumVis { x: 31; y: 49; face: root.face; mini: false }
    Hotspot { x: 29; y: 47; width: 80; height: 25; tip: root.face.visual.tip || "Click: spectrum, oscilloscope, off (V menu has more)"; onActivated: root.face.cycleVis() }

    // Title line: 1-bit Chicago, sentence case. Titles scroll; a short message that
    // does not fit wraps onto the second line instead.
    readonly property var names: [root.m.identity, root.m.sourceLabel].concat(root.face.link.snap ? root.face.link.snap.outputs.map((x) => x.name) : [])
    readonly property string mtext: Case.sentence(root.m.marquee.text, root.names)
    readonly property int lineW: 168
    readonly property var wrapped: root.m.marquee.fixed ? Lcd.wrap(root.mtext, root.lineW) : [root.mtext, ""]
    readonly property string shownTitle: !root.m.marquee.fixed && root.face.p.reducedMotion ? Lcd.fit(root.mtext, root.lineW) : root.wrapped[0]
    readonly property int titleW: Lcd.width(root.shownTitle)
    readonly property bool scrolls: !root.m.marquee.fixed && !root.face.p.reducedMotion && root.titleW > root.lineW
    readonly property var sc: root.scrolls ? Lcd.scroll(root.titleW, root.lineW, root.m.marquee.t0, root.face.now, false) : ({ offset: 0, loop: false, span: 0 })
    Binding { target: root.face; property: "faceAnimates"; value: root.scrolls; restoreMode: Binding.RestoreValue }
    readonly property string titleInk: root.m.status === "error" ? root.lc.err : root.lc.ink
    Item {
        x: 119; y: 28; width: root.lineW; height: 13
        clip: true
        PlatinumLcdText { x: -root.sc.offset; text: root.shownTitle; ink: root.titleInk }
        PlatinumLcdText { visible: root.sc.loop; x: -root.sc.offset + root.sc.span; text: root.shownTitle; ink: root.titleInk }
    }
    Hotspot { x: 119; y: 28; width: root.lineW; height: 14; tip: Case.sentence(root.m.marquee.tip || root.m.marquee.text, root.names) }

    // Line 2: the format ("320 kbps  44 kHz", "24 bit  96 kHz"), or the message's second line.
    readonly property var fmt: Pt.formatLine(root.m.format)
    Item {
        x: 119; y: 44; width: root.lineW; height: 13
        clip: true
        PlatinumLcdText { visible: root.wrapped[1] !== ""; text: root.wrapped[1]; ink: root.titleInk }
        PlatinumLcdText { id: fmtLeft; visible: root.wrapped[1] === "" && !!root.fmt; text: root.fmt ? root.fmt.left : ""; ink: root.lc.ink }
        PlatinumLcdText { visible: root.wrapped[1] === "" && !!root.fmt; x: Math.max(fmtLeft.width, 52) + 10; text: root.fmt ? root.fmt.right : ""; ink: root.lc.ink }
        PlatinumLcdText { visible: root.wrapped[1] === "" && !root.fmt; text: "--- kbps"; ink: root.lc.ghost }
        PlatinumLcdText { visible: root.wrapped[1] === "" && !root.fmt; x: 62; text: "-- kHz"; ink: root.lc.ghost }
    }
    // Line 3: mono and stereo annunciators, and whose audio it is.
    PlatinumLcdText { id: monoA; x: 119; y: 60; text: "mono"; ink: root.m.format.channels === 1 ? root.lc.ink : root.lc.ghost }
    PlatinumLcdText { x: 119 + monoA.width + 10; y: 60; text: "stereo"; ink: root.m.format.channels === 2 ? root.lc.ink : root.lc.ghost }
    readonly property string who: root.m.mode === "following" || root.m.console ? root.m.identity : ""
    PlatinumLcdText { id: whoA; visible: root.who !== ""; x: 286 - width; y: 60; text: Lcd.fit(root.who, 90); ink: root.lc.ink }
    Hotspot { visible: root.who !== ""; x: whoA.x; y: 60; width: whoA.width; height: 13; tip: root.m.mode === "following" ? "Following " + root.m.identity + ": the display shows its audio" : "Plays on " + root.m.identity }

    // ---------- seek ----------
    PlatinumSlider {
        id: seek
        pal: root.pa
        r: Sliders.platinum.seek
        frac: root.m.position
        fillTo: root.m.position
        fillCol: root.m.mode === "following" ? root.lc.dim : ""
        active: root.c.seek.enabled
        tip: active ? "Seek (Left and Right: 5 s)" : "Seek: " + root.c.seek.reason
        function report() {
            const d = (root.m.durationMs || 0) / 1000, f = root.face.seekDrag;
            root.face.say("SEEK TO: " + View.formatSeconds(f * d) + "/" + View.formatSeconds(d) + " (" + Math.round(f * 100) + "%)", 1000);
        }
        onBegan: { root.face.beginGesture(); root.face.seekDrag = seek.at(mouseX); report(); }
        onMoved: (f) => { if (root.face.seekDrag >= 0) { root.face.seekDrag = f; report(); } }
        onReleased: {
            if (root.face.seekDrag < 0) return;
            const f = root.face.seekDrag;
            root.face.seekDrag = -1;
            root.face.seekTo(f, root.face.gesture);
        }
    }

    // ---------- transport ----------
    Repeater {
        model: [
            { w: "prev", x: 8, wd: 30, tip: "Previous (Z)" },
            { w: "play", x: 40, wd: 30, tip: "Play (X)" },
            { w: "pause", x: 72, wd: 30, tip: "Pause (C)" },
            { w: "stop", x: 104, wd: 30, tip: "Stop (V)" },
            { w: "next", x: 136, wd: 30, tip: "Next (B)" },
        ]
        PlatinumButton {
            id: tb
            required property var modelData
            readonly property bool skip: root.m.skip !== null && (modelData.w === "prev" || modelData.w === "next")
            readonly property string secs: !skip ? "" : String(modelData.w === "prev" ? root.m.skip.backS : root.m.skip.forwardS)
            readonly property var ctl: skip ? root.c.seek : modelData.w === "pause" && root.m.paused ? root.c.play : root.c[modelData.w]
            readonly property string g: skip ? (modelData.w === "prev" ? "back" : "fwd") : modelData.w
            pal: root.pa
            x: modelData.x; y: 102; width: modelData.wd; height: 22
            active: ctl.enabled
            tip: skip ? (modelData.w === "prev" ? "Back " : "Forward ") + secs + " s" + (active ? " (" + (modelData.w === "prev" ? "Z" : "B") + ")" : ": " + ctl.reason)
                : active ? modelData.tip : modelData.tip.replace(/ \(.\)$/, "") + ": " + ctl.reason
            onBegan: root.face.beginGesture()
            onActivated: root.face.transport(modelData.w, root.face.gesture)
            readonly property int gx: Math.floor((width - Pt.glyphW(g)) / 2) + o - (skip ? 4 : 0)
            readonly property int gy: Math.floor((22 - Pt.glyphH(g)) / 2) + o - (skip ? 1 : 0)
            PlatinumRects { rects: { const out = []; Pt.glyph(out, tb.g, tb.gx, tb.gy, tb.active ? root.pa.glyph : root.pa.off); return out; } }
            PlatinumText { visible: tb.skip; face: root.face; size: 10; text: tb.secs; color: tb.active ? root.pa.glyph : root.pa.off; x: tb.gx + 4 + Pt.glyphW(tb.g) - 1; ty: tb.gy + 1 - 3 }
        }
    }
    PlatinumButton {
        id: ej
        pal: root.pa
        x: 170; y: 102; width: 24; height: 22
        tip: "Library (L)"
        onActivated: root.face.showView("library")
        PlatinumRects { rects: { const out = []; Pt.glyph(out, "eject", Math.floor((24 - 9) / 2) + ej.o, Math.floor((22 - 8) / 2) + ej.o, root.pa.glyph); return out; } }
    }

    // ---------- volume (the system volume) and balance or speed ----------
    PlatinumText { face: root.face; size: 10; text: "Vol"; color: root.pa.text; x: 200; ty: 100 }
    property real volShown: root.face.volumeDrag >= 0 ? root.face.volumeDrag : root.m.volume === null ? 0.5 : root.m.volume
    PlatinumSlider {
        id: vol
        pal: root.pa
        r: Sliders.platinum.volume
        frac: root.volShown
        active: root.c.volume.enabled
        tip: root.m.volumeTip
        onBegan: { root.face.volumeDrag = vol.at(mouseX); volSend.restart(); }
        onMoved: (f) => { if (root.face.volumeDrag >= 0) { root.face.volumeDrag = f; root.face.say("VOLUME: " + Math.round(f * 100) + "%", 1000); if (!volSend.running) volSend.restart(); } }
        onReleased: { if (root.face.volumeDrag >= 0) root.face.setVolume(root.face.volumeDrag); root.face.volumeDrag = -1; volSend.stop(); }
        Timer { id: volSend; interval: 120; onTriggered: if (root.face.volumeDrag >= 0) root.face.setVolume(root.face.volumeDrag) }
    }
    readonly property bool speed: root.m.spoken
    readonly property var balCtl: root.speed ? root.c.speed : root.c.balance
    readonly property real balB: root.face.balanceDrag > -2 ? root.face.balanceDrag : root.face.p.balance
    readonly property real balSp: root.face.speedDrag > 0 ? root.face.speedDrag : root.m.speed === null ? 1 : root.m.speed
    PlatinumText { face: root.face; size: 10; text: root.speed ? "Speed" : "Bal"; color: root.pa.text; x: 200; ty: 114 }
    PlatinumSlider {
        id: bal
        pal: root.pa
        r: root.speed ? Sliders.platinum.speed : Sliders.platinum.balance
        frac: root.speed ? View.speedFraction(root.balSp) : (root.balB + 1) / 2
        fillTo: root.speed ? View.speedFraction(root.balSp) : null
        fillCol: root.speed ? root.pa.speed : ""
        notch: root.speed ? View.speedFraction(1) : 0.5
        active: root.balCtl.enabled
        tip: !active ? (root.speed ? "Speed: " : "") + root.balCtl.reason
            : root.speed ? "Speed " + root.balSp.toFixed(1) + "x (synced with Ossicle)" : "Balance: " + View.balanceText(root.balB).replace("BALANCE: ", "").toLowerCase()
        function update(f) {
            if (root.speed) { root.face.speedDrag = View.speedAt(f); root.face.say("SPEED: " + root.face.speedDrag.toFixed(1) + "X", 1000); }
            else { root.face.balanceDrag = View.balanceAt(f); root.face.say(View.balanceText(root.face.balanceDrag), 1000); }
        }
        onBegan: update(bal.at(mouseX))
        onMoved: (f) => update(f)
        onReleased: {
            if (root.face.speedDrag > 0) root.face.setSpeed(root.face.speedDrag);
            if (root.face.balanceDrag > -2) root.face.setBalance(root.face.balanceDrag);
            root.face.speedDrag = -1; root.face.balanceDrag = -2;
        }
    }

    // ---------- sticky toggles with lamps, and the mark ----------
    component Toggle: PlatinumButton {
        id: tg
        property string label: ""
        property bool lit: false
        pal: root.pa
        y: 128; height: 17
        on: lit && active
        PlatinumRects { rects: { const out = []; Pt.lamp(out, root.pa, 5 + tg.o, 6 + tg.o, tg.lit, !tg.active); return out; } }
        PlatinumText { face: root.face; size: 11; text: tg.label; color: tg.active ? root.pa.text : root.pa.off; x: 14 + tg.o; ty: 2 + tg.o }
    }
    Toggle {
        x: 8; width: 66; label: "Shuffle"
        lit: root.m.shuffle === true
        active: root.c.shuffle.enabled
        tip: active ? "Shuffle (S)" : "Shuffle: " + root.c.shuffle.reason
        onBegan: root.face.beginGesture()
        onActivated: root.face.toggleShuffle(root.face.gesture)
    }
    Toggle {
        x: 77; width: 74; label: root.m.repeat === "one" ? "Repeat 1" : "Repeat"
        lit: root.m.repeat === "all" || root.m.repeat === "one"
        active: root.c.repeat.enabled
        tip: active ? "Repeat: " + ({ off: "off", all: "all", one: "one track" })[root.m.repeat] + " (R)" : "Repeat: " + root.c.repeat.reason
        onBegan: root.face.beginGesture()
        onActivated: root.face.cycleRepeat(root.face.gesture)
    }
    Toggle { x: 170; width: 42; label: "EQ"; lit: root.face.p.eqOpen; tip: "Equalizer (Alt+G)"; onActivated: root.face.prefs.set({ eqOpen: !root.face.p.eqOpen }) }
    Toggle { x: 215; width: 52; label: "List"; lit: root.face.p.plOpen; tip: "Playlist (Alt+E)"; onActivated: root.face.prefs.set({ plOpen: !root.face.p.plOpen }) }
    PlatinumButton {
        id: star
        pal: root.pa
        x: 273; y: 128; width: 19; height: 17
        tip: "About Pyxamp"
        onActivated: root.face.say("PYXAMP: PYXIS, JELLYFIN, OSSICLE", 3000)
        PlatinumRects { rects: { const out = []; Pt.glyph(out, "star", 5 + star.o, 4 + star.o, root.pa.star); Pt.P(out, 9 + star.o, 8 + star.o, root.pa.starHi); return out; } }
    }
}
