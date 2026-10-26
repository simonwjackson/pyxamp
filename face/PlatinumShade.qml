import QtQuick
import "lib/platinum.js" as Pt
import "lib/lcdtext.js" as Lcd
import "lib/lcdcase.js" as Case

// Windowshade on the Platinum face (300 x 18 sp): the collapse box folds the
// window to its title bar, which holds a mini LCD (visualizer, title, time)
// and the mini transport. The same targets as the Winamp face's shade.
Item {
    id: root
    required property var face
    property bool active: true
    readonly property var pa: face.pal
    readonly property var lc: face.lcdSet
    readonly property var m: face.m
    width: 300
    height: 18

    Rectangle { width: 300; height: 18; color: root.pa.face }
    PlatinumTitle { face: root.face; active: root.active; showTitle: false }

    // The mini LCD.
    PlatinumRects { rects: { const o = []; Pt.R(o, 22, 3, 170, 12, root.pa.flat ? root.pa.edge : "#000000"); Pt.R(o, 23, 4, 168, 10, root.lc.bg); return o; } }
    PlatinumVis { x: 25; y: 5; face: root.face; mini: true }
    Hotspot { x: 23; y: 4; width: 33; height: 10; tip: root.face.visual.tip || "Click: spectrum, oscilloscope, off"; onActivated: root.face.cycleVis() }
    readonly property var names: [root.m.identity, root.m.sourceLabel].concat(root.face.link.snap ? root.face.link.snap.outputs.map((x) => x.name) : [])
    readonly property string full: Case.sentence(root.m.marquee.text, root.names)
    readonly property string time: root.m.shortClock.replace("H", "h")
    Item {
        x: 59; y: 4; width: 94; height: 10
        clip: true
        PlatinumLcdText { y: -1; text: Lcd.fit(root.full, 94); ink: root.m.status === "error" ? root.lc.err : root.lc.ink }
    }
    Hotspot { x: 59; y: 3; width: 94; height: 12; tip: root.full }
    PlatinumLcdText { x: 188 - width; y: 3; text: root.time; ink: root.lc.ink; visible: root.time !== "" }
    Hotspot { x: 155; y: 3; width: 36; height: 12; tip: "Elapsed or remaining (Ctrl+T)"; onActivated: root.face.toggleTimeMode() }

    // The mini transport.
    Repeater {
        model: ["prev", "play", "pause", "stop", "next"]
        PlatinumButton {
            id: mb
            required property string modelData
            required property int index
            readonly property var ctl: root.m.controls[modelData === "pause" && root.m.paused ? "play" : modelData]
            pal: root.pa
            x: 197 + index * 14; y: 3; width: 13; height: 11
            active: ctl.enabled
            tip: active ? ({ prev: "Previous (Z)", play: "Play (X)", pause: "Pause (C)", stop: "Stop (V)", next: "Next (B)" })[modelData] : ctl.reason
            onBegan: root.face.beginGesture()
            onActivated: root.face.transport(modelData, root.face.gesture)
            PlatinumRects { rects: { const out = []; const g = "m" + mb.modelData; Pt.glyph(out, g, 4 + mb.o - (g === "mstop" || g === "mpause" ? 0 : 0), 3 + mb.o, mb.active ? root.pa.glyph : root.pa.off); return out; } }
        }
    }
}
