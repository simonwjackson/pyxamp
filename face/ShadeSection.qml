import QtQuick
import "lib/view.js" as View

// Windowshade (275 x 14 sp): mini vis, the first 20 characters of the status
// line, the time, and the mini transport.
Item {
    id: root
    required property var face
    property bool active: true
    readonly property var m: face.m
    width: 275
    height: 14

    SkinSprite { name: root.active ? "shade-bg-on" : "shade-bg-off" }
    TitleDrag { x: 20; y: 0; width: 218; height: 14; face: root.face }
    Hotspot { x: 6; y: 3; width: 11; height: 11; tip: "Main menu"; onActivated: root.face.openMenu("main", this) }
    Visualizer { x: 23; y: 5; face: root.face; mini: true }
    Hotspot { x: 22; y: 4; width: 40; height: 7; tip: root.face.visual.tip || "Click: spectrum, oscilloscope, off"; onActivated: root.face.cycleVis() }
    Item {
        x: 67; y: 5; width: 103; height: 6
        clip: true
        readonly property string s: View.toText5(root.m.marquee.text)
        Text5 { text: parent.s.length > 20 ? parent.s.slice(0, 19) + "." : parent.s; colour: root.m.lcd }
    }
    Hotspot { x: 66; y: 4; width: 104; height: 7; tip: root.m.marquee.tip || root.m.marquee.text }
    Text5 { x: 174; y: 5; text: root.m.shortClock.slice(-6).padStart(6, " "); colour: root.m.lcd }
    Hotspot { x: 173; y: 4; width: 32; height: 7; tip: "Elapsed or remaining (Ctrl+T)"; onActivated: root.face.toggleTimeMode() }
    Repeater {
        model: ["prev", "play", "pause", "stop", "next"]
        Hotspot {
            required property string modelData
            required property int index
            readonly property var ctl: root.m.controls[modelData === "pause" && root.face.link.player && root.face.link.player.transport === "paused" ? "play" : modelData]
            x: 207 + index * 6; y: 3; width: 6; height: 9
            active: ctl.enabled
            tip: active ? modelData[0].toUpperCase() + modelData.slice(1) : ctl.reason
            onBegan: root.face.beginGesture()
            onActivated: root.face.transport(modelData, root.face.gestureTarget)
            SkinSprite { x: 1; y: 2; name: "mini3-" + parent.modelData + (parent.active ? "-n" : "-d") }
        }
    }
    TitleButton { x: 243; y: 3; kind: "min"; tip: "Hide (SUPER+Z)"; onActivated: root.face.hide() }
    TitleButton { x: 253; y: 3; kind: "unshade"; tip: "Full size (Alt+W)"; onActivated: root.face.prefs.set({ shade: false }) }
    TitleButton { x: 263; y: 3; kind: "close"; tip: "Hide; the daemon keeps running"; onActivated: root.face.hide() }
}
