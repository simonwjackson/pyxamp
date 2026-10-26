import QtQuick
import "lib/lcdtext.js" as Lcd
import "lib/lcdfont.js" as LcdFont

// A line of LCD type: ChicagoFLF 12 in 1 bit (assets/lcd-font.png, made by
// tools/gen-lcd-font), one font pixel per skin pixel, in one of the LCD inks.
// y is the top of the 13-sp line; the baseline is 10 sp under it.
Item {
    id: root
    property string text: ""
    property string ink: "#000000"
    width: Lcd.width(root.text)
    height: 13
    Repeater {
        model: Lcd.cells(root.text, root.ink)
        Image {
            required property var modelData
            x: modelData.x
            width: LcdFont.cell
            height: LcdFont.line
            source: Qt.resolvedUrl("assets/lcd-font.png")
            sourceClipRect: Qt.rect(modelData.sx, modelData.sy, LcdFont.cell, LcdFont.line)
            smooth: false
            cache: true
        }
    }
}
