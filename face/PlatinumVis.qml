import QtQuick
import "lib/platinum.js" as Pt
import "lib/lcdtext.js" as Lcd
import "lib/vis.js" as Vis

// The Platinum face's visualizer on the LCD: 19 single-ink bars of 7 segments
// (3 x 2 sp, 3-sp pitch) with a mid-tone peak segment, or the oscilloscope;
// mini (windowshade): 10 bars of 3 segments. It draws face.visual, the same
// samples and physics (lib/vis.js) as the Winamp face; ghost segments stay lit
// faintly, as on a real LCD.
Item {
    id: root
    required property var face
    property bool mini: false
    readonly property var lc: face.lcdSet
    readonly property var v: face.visual
    readonly property string mode: face.reducedOff ? "off" : face.p.vis
    readonly property int segs: mini ? 3 : 7
    readonly property int bars: mini ? 10 : 19
    readonly property int bw: mini ? 2 : 3
    readonly property int step: bw + 1
    readonly property int h: segs * 3 - 1
    readonly property bool spectrum: mode === "spec" && (v._tag === "signal" || v._tag === "silent")
    width: bars * step - 1
    height: h

    Repeater {
        model: root.bars * root.segs
        Rectangle {
            required property int index
            readonly property int bar: Math.floor(index / root.segs)
            readonly property int seg: index % root.segs
            // The mini display samples the 19 bands at 10 places.
            readonly property int src: root.mini ? Math.min(18, Math.round(bar * 18 / (root.bars - 1))) : bar
            readonly property int lit: root.spectrum ? Math.round(root.face.visState.bars[src] / 16 * root.segs) : 0
            readonly property int peak: root.spectrum && !root.mini ? Math.round(root.face.visState.peaks[src] / 16 * root.segs) : 0
            x: bar * root.step; y: root.h - 2 - seg * 3; width: root.bw; height: 2
            antialiasing: false
            color: seg < lit ? root.lc.ink : peak > lit && seg === peak - 1 ? root.lc.mid : root.lc.ghost
        }
    }
    // Console mode: the audio plays elsewhere, a flat line in every mode.
    Rectangle { visible: root.v._tag === "flat"; y: root.h - 2; width: root.width; height: 2; color: root.lc.mid }
    // VIS OFF (reduced motion) or NO CAPTURE, in the mid tone.
    PlatinumLcdText {
        visible: root.v._tag === "label"
        text: root.v._tag === "label" ? (root.mini ? root.v.mini : root.v.text).toLowerCase() : ""
        ink: root.lc.mid
        x: Math.floor((root.width - width) / 2)
        y: Math.floor((root.h - 11) / 2) - 1
    }
    // Oscilloscope: the waveform, or a flat line with no signal.
    Rectangle { visible: root.mode === "osc" && root.v._tag === "silent"; y: Math.floor(root.h / 2); width: root.width; height: 1; color: root.lc.ink }
    Repeater {
        model: root.mode === "osc" && root.v._tag === "signal"
            ? Vis.scopeSegments(Vis.scopeRows(root.v.waveform).map((r) => Math.round(r * (root.h - 1) / 15))).slice(0, root.mini ? 2 * root.width : root.width) : []
        Rectangle {
            required property var modelData
            x: root.mini ? Math.floor(modelData.x / 2) : modelData.x
            y: modelData.top; width: 1; height: modelData.height
            color: root.lc.ink
        }
    }
}
