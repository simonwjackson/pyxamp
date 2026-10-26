import QtQuick
import "lib/platinum.js" as Pt

// The Platinum window's 16-sp windoid title bar. Each box is an existing
// action: the close box hides Pyxamp (Super+Z), the zoom box is Ctrl+D, the
// collapse box is windowshade (Alt+W). Dragging the bar moves the column; a
// double click is windowshade, a right click opens the main menu, as the
// Winamp face's title bar does. Inactive (no keyboard focus): no stripes and
// a grey title, as a Mac OS 8 window in the background.
Item {
    id: root
    required property var face
    property bool active: true
    property bool showTitle: true
    readonly property var pa: face.pal
    width: 300
    height: 18

    PlatinumText { id: title; face: root.face; visible: root.showTitle; text: "Pyxamp"; ty: 3; x: Math.round(150 - implicitWidth / 2); color: root.active ? root.pa.text : root.pa.off }
    PlatinumRects {
        z: -1
        rects: Pt.titleBar(root.pa, root.showTitle && root.active ? Math.ceil(title.implicitWidth) : 0,
                           closeBox.down ? "close" : zoomBox.down ? "zoom" : collapseBox.down ? "collapse" : "", root.face.p.dbl)
    }
    TitleDrag {
        x: 0; y: 0; width: 300; height: 17; face: root.face
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: (mouse) => { if (mouse.button === Qt.RightButton) root.face.openMenu("main", menuAt); }
    }
    Item { id: menuAt; x: 21; y: 17; width: 1; height: 0 }
    Hotspot { id: closeBox; x: 4; y: 1; width: 15; height: 16; tip: "Hide Pyxamp; playback goes on (Super+Z)"; onActivated: root.face.hide() }
    Hotspot {
        id: zoomBox
        x: 267; y: 1; width: 15; height: 16
        tip: root.face.p.dbl ? "Zoom: normal size (Ctrl+D)" : root.face.planFor(true).refused ? "Double size does not fit this screen" : "Zoom: double size (Ctrl+D)"
        onActivated: root.face.toggleDouble()
    }
    Hotspot {
        id: collapseBox
        x: 281; y: 1; width: 16; height: 16
        tip: root.face.p.shade ? "Expand (Alt+W)" : "Collapse to the title bar (Alt+W)"
        onActivated: root.face.prefs.set({ shade: !root.face.p.shade })
    }
}
