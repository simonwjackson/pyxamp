import QtQuick
import "lib/text.js" as TextLayout

// A line in W95FA Grid at one font pixel per skin pixel (13-sp line, baseline
// at 10). Used by the playlist, menus and tooltips. colour: normal, current,
// dim, err, black, grey.
Item {
    id: root
    property string text
    property string colour: "normal"
    width: TextLayout.sysWidth(text)
    height: 13
    Repeater {
        model: TextLayout.sysCells(root.text, root.colour)
        Image {
            required property var modelData
            x: modelData.x
            width: modelData.w
            height: 13
            source: Qt.resolvedUrl("assets/skin.png")
            sourceClipRect: Qt.rect(modelData.sx, modelData.sy, modelData.w, 13)
            smooth: false
            cache: true
        }
    }
}
