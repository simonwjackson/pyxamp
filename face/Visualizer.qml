import QtQuick
import "lib/sprites.js" as Sprites
import "lib/vis.js" as Vis

// The visualizer area: spectrum (19 thick bands with peaks), oscilloscope, or a
// label. Full size 76 x 16 sp; mini (windowshade) 38 x 5 sp.
// It draws only the daemon's samples for the followed player (face.visual).
Item {
    id: root
    required property var face
    property bool mini: false
    readonly property int h: mini ? 5 : 16
    readonly property var v: face.visual
    readonly property string mode: face.reducedOff ? "off" : face.p.vis
    readonly property var lcd: Sprites.lcd[face.m.lcd]
    width: mini ? 38 : 76
    height: h

    // VIS OFF (reduced motion) or NO CAPTURE (capture unavailable).
    Text5 {
        visible: root.v._tag === "label"
        readonly property string s: root.v._tag === "label" ? (root.mini ? root.v.mini : root.v.text) : ""
        text: s
        colour: root.face.m.lcd + "Mid"
        x: Math.floor((root.width - (s.length * 5 - 1)) / 2)
        y: Math.floor((root.h - 5) / 2)
    }

    // Spectrum: bars fall and peaks hold even after the signal stops.
    Repeater {
        model: root.mode === "spec" && (root.v._tag === "signal" || root.v._tag === "silent") ? 19 : 0
        Item {
            required property int index
            readonly property int bh: root.mini ? Math.round(root.face.visState.bars[index] / 16 * 5) : Math.round(root.face.visState.bars[index])
            readonly property int pk: root.mini ? 0 : Math.round(root.face.visState.peaks[index])
            x: root.mini ? index * 2 : index * 4
            width: root.mini ? 1 : 3
            height: root.h
            Item {
                y: root.h - parent.bh; width: parent.width; height: parent.bh
                clip: true
                SkinSprite { y: -(root.h - parent.height); name: root.mini ? "vis-mini" : "vis-bar" }
            }
            Rectangle { y: root.h - parent.pk; width: 3; height: 1; color: Sprites.visPeak; visible: parent.pk > 0 }
        }
    }

    // Oscilloscope: the waveform, or a flat line when there is no signal.
    Rectangle {
        visible: root.mode === "osc" && root.v._tag === "silent"
        y: Math.floor(root.h / 2); width: root.width; height: 1
        color: "#ffffff"
    }
    Repeater {
        model: root.mode === "osc" && root.v._tag === "signal" ? Vis.scopeSegments(Vis.scopeRows(root.v.waveform).map((r) => root.mini ? Math.round(r * 4 / 15) : r)).slice(0, root.width) : []
        Item {
            required property var modelData
            x: root.mini ? Math.floor(modelData.x / 2) : modelData.x
            y: modelData.top; width: 1; height: modelData.height
            clip: true
            SkinSprite { y: -parent.y; name: root.mini ? "vis-mini" : "vis-scope" }
        }
    }
}
