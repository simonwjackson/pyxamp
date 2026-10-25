import QtQuick
import Quickshell
import "lib/sprites.js" as Sprites
import "lib/text.js" as TextLayout
import "lib/tokens.js" as Tokens

// The yellow tooltip, at the surface's scale, below and right of the pointer.
// An xdg popup, so it may extend past the column. It never takes focus.
PopupWindow {
    id: tip
    property var host: null
    property real scaleFactor: 1
    readonly property int w: TextLayout.sysWidth(Tips.text) + 8
    visible: Tips.text !== "" && Tips.anchor !== null
    color: "transparent"
    anchor.item: Tips.anchor
    anchor.rect.x: Tips.px + 6
    anchor.rect.y: Tips.py + 16
    anchor.edges: Edges.Top | Edges.Left
    anchor.gravity: Edges.Bottom | Edges.Right
    implicitWidth: Math.max(1, Math.ceil(tip.w * scaleFactor))
    implicitHeight: Math.ceil(16 * scaleFactor)

    Item {
        width: tip.w
        height: 16
        transformOrigin: Item.TopLeft
        scale: tip.scaleFactor
        Rectangle { anchors.fill: parent; color: Tokens.tipBorder }
        Rectangle { x: 1; y: 1; width: parent.width - 2; height: 14; color: Sprites.palette.tip }
        SysText { x: 4; y: 1; text: Tips.text; colour: "black" }
    }
}
