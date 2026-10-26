import QtQuick

// One Platinum menu, drawn the way the desktop's bar draws its menus, in
// logical px at the monitor's own scale (Ctrl+D does not double it). Pyxamp is
// its own Quickshell config and cannot import the bar's files, so this
// reproduces mountainous modules/quickshell/PlatinumCascadeMenu.qml (Panel and
// MenuRow), PlatinumMenuFrame.qml and PlatinumBevel.qml (cairn branch, read
// 2026-10-26): ChicagoFLF 14 with 0.3 tracking, 26-px rows, labels at 38, key
// hints in ChicagoFLF 12 on the right, the #333399 (dark: #5858D0)
// highlight, an etched separator, a black edge, a one-rule bevel and a hard
// shadow two rules down and right. Every rule is `hair` thick: one physical
// pixel row per rounded scale step (PlatinumTokens.hair).
// The same API as MenuPanel.qml: items, hover, rowsY, hovered, picked.
Item {
    id: menu
    required property var face
    property var items: []
    property int hover: -1
    signal hovered(int index)
    signal picked(int index)
    readonly property var pa: face.pal
    readonly property real dpr: face.monitorScale > 0 ? face.monitorScale : 1
    readonly property real hair: Math.max(1, Math.round(dpr)) / dpr

    // PlatinumTokens: menuTextLeft 30 + menuGap 8, menuRightInset 12, menuLine 26.
    readonly property real labelX: 38
    readonly property real rowH: 26
    readonly property real sepH: 4 + 2 * hair + 5
    FontMetrics { id: fm14; font.family: menu.face.chicago; font.pixelSize: 14 }
    FontMetrics { id: fm12; font.family: menu.face.chicago; font.pixelSize: 12 }
    readonly property real naturalWidth: {
        let w = 200;
        for (const it of menu.items) {
            if (it.sep) continue;
            const t = (it.key ? fm12.advanceWidth(it.key) + 8 : 0) + (it.sub ? 7 + 8 : 0);
            w = Math.max(w, menu.labelX + fm14.advanceWidth(it.l || "") + 0.3 * (it.l || "").length + 8 + t + 12 + 2 * menu.hair);
        }
        return Math.ceil(w / 2) * 2;
    }
    readonly property var rowsY: {
        const out = []; let y = menu.hair + 4;
        for (const it of menu.items) { out.push(y); y += it.sep ? menu.sepH : menu.rowH; }
        return out;
    }
    readonly property real contentH: { let h = 0; for (const it of menu.items) h += it.sep ? menu.sepH : menu.rowH; return h; }
    width: Math.max(120, Math.min(menu.naturalWidth, 380)) + 2 * hair
    height: menu.contentH + 4 + 5 + 2 * hair + 2 * hair

    // The hard shadow, then the box: black edge, one-rule bevel.
    Rectangle { x: 2 * menu.hair; y: 2 * menu.hair; width: parent.width - 2 * menu.hair; height: parent.height - 2 * menu.hair; color: menu.pa.menuShadow }
    Rectangle {
        id: box
        width: parent.width - 2 * menu.hair; height: parent.height - 2 * menu.hair
        color: menu.pa.menu
        border.width: menu.hair; border.color: menu.pa.menuEdge
        Rectangle { x: menu.hair; y: menu.hair; width: box.width - 2 * menu.hair; height: menu.hair; color: menu.pa.menuHi }
        Rectangle { x: menu.hair; y: menu.hair; width: menu.hair; height: box.height - 2 * menu.hair; color: menu.pa.menuHi }
        Rectangle { x: menu.hair; y: box.height - 2 * menu.hair; width: box.width - 2 * menu.hair; height: menu.hair; color: menu.pa.menuLo }
        Rectangle { x: box.width - 2 * menu.hair; y: menu.hair; width: menu.hair; height: box.height - 2 * menu.hair; color: menu.pa.menuLo }
    }

    Repeater {
        model: menu.items
        Item {
            id: row
            required property var modelData
            required property int index
            readonly property bool lit: menu.hover === index && !modelData.dis && !modelData.sep
            readonly property color fg: modelData.dis ? menu.pa.menuOff : lit ? "#ffffff" : menu.pa.menuText
            readonly property color fg2: modelData.dis ? menu.pa.menuOff : lit ? "#ffffff" : menu.pa.menuText2
            x: menu.hair; y: menu.rowsY[index] || 0
            width: box.width - 2 * menu.hair; height: modelData.sep ? menu.sepH : menu.rowH

            Rectangle { visible: !!row.modelData.sep; y: 4; width: parent.width; height: menu.hair; color: menu.pa.menuLo }
            Rectangle { visible: !!row.modelData.sep; y: 4 + menu.hair; width: parent.width; height: menu.hair; color: menu.pa.menuHi }
            Rectangle { anchors.fill: parent; visible: row.lit; color: menu.pa.menuSel }
            // The gutter: a check mark or a radio dot (ChicagoFLF has neither glyph).
            Canvas {
                visible: !!row.modelData.check
                x: Math.round((30 - 12) / 2 - 2); y: Math.round((menu.rowH - 12) / 2); width: 12; height: 12
                property color ink: row.fg
                onInkChanged: requestPaint()
                onPaint: {
                    const g = getContext("2d"); g.reset();
                    g.strokeStyle = ink; g.lineWidth = 2; g.lineCap = "square";
                    g.beginPath(); g.moveTo(1.5, 6.5); g.lineTo(4.5, 10); g.lineTo(10.5, 1.5); g.stroke();
                }
            }
            Rectangle {
                visible: !!row.modelData.radio
                x: Math.round(30 / 2 - 2 - 3.5); y: Math.round(menu.rowH / 2 - 3.5); width: 7; height: 7; radius: 3.5
                color: row.fg
            }
            Text {
                x: menu.labelX - menu.hair
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(0, trail.x - x - 8)
                text: row.modelData.l || ""
                textFormat: Text.PlainText
                elide: Text.ElideRight
                visible: !row.modelData.sep
                color: row.fg
                font.family: menu.face.chicago
                font.pixelSize: 14
                font.letterSpacing: 0.3
            }
            Row {
                id: trail
                visible: !row.modelData.sep
                anchors.right: parent.right
                anchors.rightMargin: 12 - 4
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                Text {
                    visible: !!row.modelData.key
                    anchors.verticalCenter: parent.verticalCenter
                    text: row.modelData.key || ""
                    color: row.fg2
                    font.family: menu.face.chicago
                    font.pixelSize: 12
                    font.letterSpacing: 0.3
                }
                Canvas {
                    visible: !!row.modelData.sub
                    anchors.verticalCenter: parent.verticalCenter
                    width: 7; height: 10
                    property color ink: row.fg
                    onInkChanged: requestPaint()
                    onPaint: { const g = getContext("2d"); g.reset(); g.fillStyle = ink; g.beginPath(); g.moveTo(0, 0); g.lineTo(6, 5); g.lineTo(0, 10); g.closePath(); g.fill(); }
                }
            }
            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                enabled: !row.modelData.sep
                onEntered: menu.hovered(row.index)
                // Picks on press, as MenuPanel.qml (a virtual pointer's release reaches
                // a layer surface's popup late under Hyprland 0.56).
                onPressed: menu.picked(row.index)
            }
        }
    }
    MouseArea { z: -1; anchors.fill: parent }
}
