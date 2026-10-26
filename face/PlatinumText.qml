import QtQuick

// Chrome text: ChicagoFLF drawn smooth at its size in sp (12, 11 or 10), so it
// is 12k physical px at scale k, as the desktop's bar draws it. `base` is the
// baseline (sp); the mock places a 12-sp line's baseline 10 sp under its top.
Text {
    id: root
    required property var face
    property real ty: 0
    property int size: 12
    y: root.ty + Math.round(root.size * 0.82) - root.baselineOffset
    font.family: root.face.chicago
    font.pixelSize: root.size
    textFormat: Text.PlainText
    renderType: Text.QtRendering
}
