import QtQuick
import Quickshell

// Balloon Help, the Platinum face's tooltip: a rounded balloon with a tail
// that points at the pointer, ChicagoFLF 12, white in light and dark grey
// otherwise (PlatinumTokens.balloon). It is an xdg popup of the surface, drawn
// at the monitor's own scale, so Ctrl+D does not double it. It reads the same
// tip channel as the Winamp face's tooltip (Tips.qml) and never takes focus.
PopupWindow {
    id: tip
    required property var face
    property real scaleFactor: 1              // logical px per sp of the anchor (to place it)
    readonly property var pa: face.pal
    readonly property real w: Math.ceil(label.implicitWidth) + 20
    readonly property real h: 24
    readonly property real tail: 10
    visible: Tips.text !== "" && Tips.anchor !== null
    color: "transparent"
    anchor.item: Tips.anchor
    // The tail's tip sits 14 px right of the balloon's left edge, at the pointer.
    anchor.rect.x: Tips.px - 14 / Math.max(0.01, tip.scaleFactor)
    anchor.rect.y: Tips.py + 2 / Math.max(0.01, tip.scaleFactor)
    anchor.edges: Edges.Top | Edges.Left
    anchor.gravity: Edges.Bottom | Edges.Right
    implicitWidth: Math.max(1, Math.ceil(tip.w + 4))
    implicitHeight: Math.ceil(tip.h + tip.tail + 4)

    Canvas {
        id: art
        anchors.fill: parent
        property color fill: tip.pa.balloon
        property color edge: tip.pa.balloonEdge
        property real bw: tip.w
        onFillChanged: requestPaint()
        onEdgeChanged: requestPaint()
        onBwChanged: requestPaint()
        onPaint: {
            const g = getContext("2d"), x = 0.5, y = tip.tail + 0.5, w = tip.w - 1, h = tip.h - 1, r = 8;
            g.reset();
            function shape(dx, dy) {
                g.beginPath();
                g.moveTo(x + dx + r, y + dy); g.lineTo(x + dx + 10, y + dy); g.lineTo(x + dx + 14, y + dy - tip.tail); g.lineTo(x + dx + 22, y + dy);
                g.lineTo(x + dx + w - r, y + dy); g.quadraticCurveTo(x + dx + w, y + dy, x + dx + w, y + dy + r);
                g.lineTo(x + dx + w, y + dy + h - r); g.quadraticCurveTo(x + dx + w, y + dy + h, x + dx + w - r, y + dy + h);
                g.lineTo(x + dx + r, y + dy + h); g.quadraticCurveTo(x + dx, y + dy + h, x + dx, y + dy + h - r);
                g.lineTo(x + dx, y + dy + r); g.quadraticCurveTo(x + dx, y + dy, x + dx + r, y + dy);
                g.closePath();
            }
            shape(2, 2); g.fillStyle = "rgba(0,0,0,0.35)"; g.fill();
            shape(0, 0); g.fillStyle = fill; g.fill(); g.strokeStyle = edge; g.lineWidth = 1; g.stroke();
        }
    }
    Text {
        id: label
        x: 10; y: tip.tail + Math.round((tip.h - height) / 2)
        text: Tips.text
        textFormat: Text.PlainText
        color: tip.pa.balloonText
        font.family: tip.face.chicago
        font.pixelSize: 12
        font.letterSpacing: 0.3
    }
}
