import QtQuick
import "lib/platinum.js" as Pt
import "lib/drag.js" as Drag

// A horizontal Platinum slider: a 6-sp groove, an optional progress fill and
// notch, and the pointed thumb. The pointer target is the control's own
// rectangle (lib/sliders.js), and a drag maps from it (lib/drag.js), so the
// same gesture means the same value wherever the slider sits.
Hotspot {
    id: root
    required property var pal
    required property var r                 // the target: { x, y, w, h, grip, groove }
    property var frac: null                 // 0..1, or null: no thumb
    property var fillTo: null
    property string fillCol: ""
    property var notch: null
    property bool dim: false
    signal moved(real f)
    x: r.x; y: r.y; width: r.w; height: r.h
    function at(mx) { return Drag.fraction(mx, root.width, root.r.grip); }
    PlatinumRects {
        z: -1
        rects: {
            const out = [], g = root.r.groove;
            Pt.hslider(out, root.pal, g.x - root.r.x, g.y - root.r.y, g.w,
                       { frac: root.frac, fillTo: root.fillTo, fillCol: root.fillCol, notch: root.notch, pressed: root.pressed && root.active, dim: root.dim || !root.active });
            return out;
        }
    }
    onPositionChanged: (mouse) => { if (pressed && active) root.moved(root.at(mouse.x)); }
}
