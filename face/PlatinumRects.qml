import QtQuick

// Draws a list of whole-sp rectangles ({ x, y, w, h, c }, lib/platinum.js) at
// one unit per skin pixel. The Platinum face's chrome is made of these.
Item {
    id: root
    property var rects: []
    Repeater {
        model: root.rects
        Rectangle {
            required property var modelData
            x: modelData.x; y: modelData.y; width: modelData.w; height: modelData.h
            color: modelData.c
            antialiasing: false
        }
    }
}
