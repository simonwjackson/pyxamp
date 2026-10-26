import QtQuick
import "lib/platinum.js" as Pt

// The Platinum face's one window: main (or windowshade), EQ and playlist in a
// column, each folded section keeping its 17-sp disclosure header, inside a
// 1-sp window frame (docs/brief.md, Platinum face). 300 sp wide.
Item {
    id: root
    required property var face
    property bool active: true
    readonly property var pa: face.pal
    width: 300
    height: col.height + 1

    Column {
        id: col
        PlatinumMain { face: root.face; active: root.active; visible: !root.face.p.shade }
        PlatinumShade { face: root.face; active: root.active; visible: root.face.p.shade }
        PlatinumEq { face: root.face; active: root.active }
        PlatinumPlaylist { face: root.face; active: root.active; steps: root.face.fit.plN }
    }
    // The window frame, and the Platinum shade line inside its bottom edge.
    PlatinumRects {
        rects: {
            const o = [], H = root.height;
            Pt.frame(o, 0, 0, 300, H, root.pa.edge, root.pa.edge);
            if (!root.pa.flat) Pt.R(o, 1, H - 2, 298, 1, root.pa.lo);
            return o;
        }
    }
}
