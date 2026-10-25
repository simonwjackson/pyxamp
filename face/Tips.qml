pragma Singleton
import QtQuick
import Quickshell

// The one tooltip channel. Hotspots report hover here; the tooltip popup in
// PyxampPanel reads it. A tip appears 550 ms after the pointer settles and
// goes when the pointer leaves or presses.
Singleton {
    id: root
    property string text: ""
    property Item anchor: null
    property real px: 0
    property real py: 0
    property string pending: ""
    property Item pendingAnchor: null
    property real pendingX: 0
    property real pendingY: 0

    function hover(item, tip, x, y) {
        if (!tip) { clear(); return; }
        if (root.anchor === item && root.text === tip) return;
        root.text = "";
        root.pending = tip; root.pendingAnchor = item; root.pendingX = x; root.pendingY = y;
        delay.restart();
    }
    function clear() { delay.stop(); root.text = ""; root.anchor = null; root.pending = ""; root.pendingAnchor = null; }

    property Timer delay: Timer {
        interval: 550
        onTriggered: { root.anchor = root.pendingAnchor; root.px = root.pendingX; root.py = root.pendingY; root.text = root.pending; }
    }
}
