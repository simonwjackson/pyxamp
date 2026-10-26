import QtQuick
import "lib/platinum.js" as Pt

// A Platinum bevel button as a pointer target: raised, sunk while pressed,
// sticky "on" (a toggle) or latched (its menu is open). Content placed inside
// moves by `o` (1 sp) while the button is sunk, as Platinum buttons do.
Hotspot {
    id: root
    required property var pal
    property bool on: false
    property bool latched: false
    readonly property bool sunk: root.down || root.latched
    readonly property int o: root.sunk || root.on ? 1 : 0
    PlatinumRects {
        z: -1
        rects: { const out = []; Pt.button(out, root.pal, 0, 0, root.width, root.height, { pressed: root.sunk, on: root.on }); return out; }
    }
}
