import QtQuick
import "lib/text.js" as TextLayout

// A line in the Pyx 5x6 font: fixed 5-sp advance, upper case only.
// colour is a row name from lib/sprites.js (green, amber, label, ...).
Item {
    id: root
    property string text
    property string colour: "green"
    width: text.length * 5
    height: 6
    Repeater {
        model: TextLayout.text5Cells(root.text, root.colour)
        Image {
            required property var modelData
            x: modelData.x
            width: 5
            height: 6
            source: Qt.resolvedUrl("assets/skin.png")
            sourceClipRect: Qt.rect(modelData.sx, modelData.sy, 5, 6)
            smooth: false
            cache: true
        }
    }
}
