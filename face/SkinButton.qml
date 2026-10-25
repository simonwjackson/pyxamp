import QtQuick

// A sprite button: normal, pressed and dimmed faces from the skin sheet.
// The dimmed face shows when the action is not available (active: false).
Hotspot {
    id: root
    property string face
    property string pressedFace: face + "-p"
    property string dimFace: face
    property bool latched: false        // drawn pressed while its menu is open
    width: art.width
    height: art.height
    SkinSprite {
        id: art
        name: !root.active ? root.dimFace : (root.down || root.latched) ? root.pressedFace : root.face
    }
}
