import QtQuick
import "lib/sprites.js" as Sprites
import "lib/view.js" as View

// The main section (275 x 116 sp) at Winamp 2's coordinates (docs/mock/pyxamp.js drawMain).
Item {
    id: root
    required property var face
    property bool active: true
    readonly property var m: face.m
    readonly property var lcd: Sprites.lcd[m.lcd]
    readonly property var c: m.controls
    width: 275
    height: 116

    SkinSprite { name: "main-bg" }
    SkinSprite { x: 1; y: 1; name: root.active ? "main-title-on" : "main-title-off" }
    TitleDrag { x: 20; y: 1; width: 222; height: 14; face: root.face }
    Hotspot { x: 6; y: 3; width: 11; height: 11; tip: "Main menu"; onActivated: root.face.openMenu("main", this) }
    TitleButton { x: 243; y: 4; kind: "min"; tip: "Hide (SUPER+Z)"; onActivated: root.face.hide() }
    TitleButton { x: 253; y: 4; kind: "shade"; tip: "Windowshade (Alt+W)"; onActivated: root.face.prefs.set({ shade: true }) }
    TitleButton { x: 263; y: 4; kind: "close"; tip: "Hide; the daemon keeps running (SUPER+Z)"; onActivated: root.face.hide() }

    // Clutterbar: O A I D V.
    Repeater {
        model: [
            { ch: "O", on: root.face.menu === "main", tip: "Options" },
            { ch: "A", on: root.face.p.onTop, tip: root.face.p.onTop ? "On top. Click: on the desktop" : "On the desktop. Click: on top" },
            { ch: "I", on: root.face.view === "info" && root.face.p.plOpen, tip: "Item info (Alt+3)" },
            { ch: "D", on: root.face.p.dbl, tip: !root.face.p.dbl && root.face.planFor(true).refused ? "Double size does not fit this screen" : "Double size (Ctrl+D)" },
            { ch: "V", on: root.face.menu === "vis", tip: "Visualizer" },
        ]
        Hotspot {
            id: cl
            required property var modelData
            required property int index
            x: 10; y: 22 + index * 8; width: 9; height: 8
            tip: modelData.tip
            onActivated: {
                switch (modelData.ch) {
                case "O": root.face.openMenu("main", cl); break;
                case "A": root.face.setLayer(!root.face.p.onTop); break;
                case "I": root.face.showView("info"); break;
                case "D": root.face.toggleDouble(); break;
                case "V": root.face.openMenu("vis", cl); break;
                }
            }
            Text5 { x: 2 + (cl.down ? 1 : 0); y: 2 + (cl.down ? 1 : 0); text: cl.modelData.ch; colour: cl.modelData.on ? "clutterOn" : "clutterOff" }
        }
    }

    // Status light and mark.
    Rectangle { x: 23; y: 28; width: 2; height: 9; color: root.m.status === "error" ? Sprites.palette.red : root.m.status === "play" ? root.lcd.on : root.lcd.dim }
    SkinSprite { x: 27; y: 28; name: "st-play-" + root.m.lcd; visible: root.m.status === "play" }
    Rectangle { x: 27; y: 29; width: 2; height: 7; color: root.lcd.on; visible: root.m.status === "pause" }
    Rectangle { x: 31; y: 29; width: 2; height: 7; color: root.lcd.on; visible: root.m.status === "pause" }
    Rectangle { x: 27; y: 29; width: 7; height: 7; visible: root.m.status === "stop" || root.m.status === "error"; color: root.m.status === "error" ? Sprites.palette.red : root.lcd.on }

    // Clock.
    Item {
        visible: root.m.clock !== null
        readonly property var ck: root.m.clock || { minus: false, digits: [0, 0, 0, 0], hours: false }
        SkinSprite { x: 36; y: 26; name: "d9---" + root.m.lcd; visible: parent.ck.minus }
        SkinSprite { x: 48; y: 26; name: "d9-" + parent.ck.digits[0] + "-" + root.m.lcd }
        SkinSprite { x: 60; y: 26; name: "d9-" + parent.ck.digits[1] + "-" + root.m.lcd }
        Rectangle { x: 72; y: 30; width: 2; height: 2; color: root.lcd.on }
        Rectangle { x: 72; y: 35; width: 2; height: 2; color: root.lcd.on }
        SkinSprite { x: 78; y: 26; name: "d9-" + parent.ck.digits[2] + "-" + root.m.lcd }
        SkinSprite { x: 90; y: 26; name: "d9-" + parent.ck.digits[3] + "-" + root.m.lcd }
        Text5 { x: 99; y: 33; text: "H"; colour: root.m.lcd + "Mid"; visible: parent.ck.hours }
    }
    Hotspot { x: 34; y: 24; width: 66; height: 16; tip: "Elapsed or remaining (Ctrl+T)"; onActivated: root.face.toggleTimeMode() }

    // Visualizer.
    Visualizer { x: 24; y: 43; face: root.face; mini: false }
    Hotspot {
        x: 22; y: 42; width: 80; height: 19
        tip: root.face.visual.tip || "Click: spectrum, oscilloscope, off"
        onActivated: root.face.cycleVis()
    }

    // Marquee: the status line.
    Item {
        x: 111; y: 27; width: 155; height: 6
        clip: true
        readonly property var frame: View.marqueeFrame(root.m.marquee.text, root.m.marquee.fixed, root.m.marquee.t0, root.face.now, root.face.p.reducedMotion)
        Text5 { x: -parent.frame.offset; text: parent.frame.text; colour: root.m.lcd }
    }
    Hotspot { x: 110; y: 24; width: 155; height: 12; tip: root.m.marquee.tip || root.m.marquee.text }

    // kbps (or bit depth), kHz, mono, stereo: only what the player reports.
    Text5 { x: 111; y: 43; text: (root.m.format.left || "").padStart(3, " "); colour: root.m.lcd }
    Text5 { x: 157; y: 43; text: (root.m.format.khz || "").padStart(3, " "); colour: root.m.lcd }
    SkinSprite { x: 131; y: 42; name: "lb-" + root.m.format.leftUnit + "-label" }
    SkinSprite { x: 177; y: 42; name: "lb-kHz-label" }
    SkinSprite { x: 212; y: 42; name: root.m.format.channels === 1 ? "lb-mono-lit" : "lb-mono-dim" }
    SkinSprite { x: 238; y: 42; name: root.m.format.channels === 2 ? "lb-stereo-lit" : "lb-stereo-dim" }

    // Volume: the followed player's own MPRIS volume.
    Item {
        id: vol
        readonly property real shown: root.face.volumeDrag >= 0 ? root.face.volumeDrag : root.m.volume === null ? 0.5 : root.m.volume
        readonly property bool on: root.c.volume.enabled
        SkinSprite { x: 108; y: 60; name: vol.on ? "vol-" + Math.round(vol.shown * (Sprites.volFrames - 1)) : "vol-d" }
        SkinSprite { x: 107 + Math.round(vol.shown * 54); y: 57; name: !vol.on ? "knob-d" : volArea.pressed ? "knob-p" : "knob" }
    }
    Hotspot {
        id: volArea
        x: 107; y: 56; width: 68; height: 13
        active: root.c.volume.enabled
        tip: active ? "Volume of " + root.m.identity + ": " + Math.round((root.m.volume || 0) * 100) + "%" : "Volume: " + root.c.volume.reason
        function valueAt(mx) { return Math.max(0, Math.min(1, (x + mx - 114) / 54)); }
        onBegan: { root.face.beginGesture(); root.face.volumeDrag = valueAt(mouseX); volSend.restart(); }
        onPositionChanged: (mouse) => { if (pressed && root.face.volumeDrag >= 0) { root.face.volumeDrag = valueAt(mouse.x); root.face.say("VOLUME: " + Math.round(root.face.volumeDrag * 100) + "%", 1000); if (!volSend.running) volSend.restart(); } }
        onReleased: { if (root.face.volumeDrag >= 0) root.face.setVolume(root.face.volumeDrag, root.face.gestureTarget); root.face.volumeDrag = -1; volSend.stop(); }
        Timer { id: volSend; interval: 120; onTriggered: if (root.face.volumeDrag >= 0) root.face.setVolume(root.face.volumeDrag, root.face.gestureTarget) }
    }

    // Balance applies to Pyxamp's own audio only: none in step 1.
    SkinSprite { x: 178; y: 60; name: "bal-d" }
    SkinSprite { x: 189; y: 57; name: "knob-d" }
    Hotspot { x: 177; y: 56; width: 38; height: 13; active: false; tip: "Balance: Pyxamp's own audio only" }

    SkinButton { x: 219; y: 58; face: "btn-eq-" + (root.face.p.eqOpen ? 1 : 0); tip: "Equalizer (Alt+G)"; onActivated: root.face.prefs.set({ eqOpen: !root.face.p.eqOpen }) }
    SkinButton { x: 242; y: 58; face: "btn-pl-" + (root.face.p.plOpen ? 1 : 0); tip: "Playlist (Alt+E)"; onActivated: root.face.prefs.set({ plOpen: !root.face.p.plOpen }) }

    // Position bar.
    SkinSprite {
        x: 16 + Math.round((root.m.position || 0) * 219); y: 72
        visible: root.m.position !== null
        name: root.face.seekDrag >= 0 ? "thumb-p" : "thumb"
    }
    Hotspot {
        x: 16; y: 72; width: 248; height: 10
        active: root.c.seek.enabled
        tip: active ? "Seek" : "Seek: " + root.c.seek.reason
        function fractionAt(mx) { return Math.max(0, Math.min(1, (x + mx - 30) / 219)); }
        function report() {
            const d = root.face.link.player.track.durationMs / 1000, f = root.face.seekDrag;
            root.face.say("SEEK TO: " + View.formatSeconds(f * d) + "/" + View.formatSeconds(d) + " (" + Math.round(f * 100) + "%)", 1000);
        }
        onBegan: { root.face.beginGesture(); root.face.seekDrag = fractionAt(mouseX); report(); }
        onPositionChanged: (mouse) => { if (pressed && root.face.seekDrag >= 0) { root.face.seekDrag = fractionAt(mouse.x); report(); } }
        onReleased: {
            if (root.face.seekDrag < 0) return;
            const f = root.face.seekDrag;
            root.face.seekDrag = -1;
            root.face.seekTo(f, root.face.gestureTarget, root.face.gestureTrack);
        }
    }

    // Transport.
    Repeater {
        model: [
            { w: "prev", x: 16, y: 88, tip: "Previous (Z)" },
            { w: "play", x: 39, y: 88, tip: "Play (X)" },
            { w: "pause", x: 62, y: 88, tip: "Pause (C)" },
            { w: "stop", x: 85, y: 88, tip: "Stop (V)" },
            { w: "next", x: 108, y: 88, tip: "Next (B)" },
        ]
        SkinButton {
            required property var modelData
            readonly property var ctl: modelData.w === "pause" && root.face.link.player && root.face.link.player.transport === "paused" ? root.c.play : root.c[modelData.w]
            x: modelData.x; y: modelData.y
            face: "tr-" + modelData.w
            dimFace: "tr-" + modelData.w + "-d"
            active: ctl.enabled
            tip: active ? modelData.tip : modelData.tip.replace(/ \(.\)$/, "") + ": " + ctl.reason
            onBegan: root.face.beginGesture()
            onActivated: root.face.transport(modelData.w, root.face.gestureTarget)
        }
    }
    SkinButton { x: 136; y: 89; face: "tr-eject"; tip: "Library (L)"; onActivated: root.face.showView("library") }
    SkinButton {
        x: 164; y: 89
        face: "btn-shuffle-" + (root.m.shuffle ? 1 : 0)
        dimFace: "btn-shuffle-d"
        active: root.c.shuffle.enabled
        tip: active ? "Shuffle (S)" : "Shuffle: " + root.c.shuffle.reason
        onBegan: root.face.beginGesture()
        onActivated: root.face.toggleShuffle(root.face.gestureTarget)
    }
    SkinButton {
        x: 211; y: 89
        face: "btn-rep-" + (root.m.repeat || "off")
        dimFace: "btn-rep-d"
        active: root.c.repeat.enabled
        tip: active ? "Repeat: " + ({ off: "off", all: "all", one: "one track" })[root.m.repeat] + " (R)" : "Repeat: " + root.c.repeat.reason
        onBegan: root.face.beginGesture()
        onActivated: root.face.cycleRepeat(root.face.gestureTarget)
    }
    SkinButton { x: 253; y: 91; face: "mark-big"; tip: "About Pyxamp"; onActivated: root.face.say("PYXAMP: FOLLOWS THE PLAYER THAT STARTED LAST", 3000) }
}
