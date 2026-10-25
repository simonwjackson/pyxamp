import QtQuick
import "lib/sprites.js" as Sprites
import "lib/text.js" as TextLayout

// One Win9x-grey menu in W95FA Grid, drawn in skin pixels (decision 15).
// items: [{ l, key?, act?, dis?, check?, radio?, sub?, sep? }] from Face.menuItems.
// It draws and reports the pointer; MenuHost owns opening, closing and keys.
Item {
    id: menu
    property var items: []
    property int hover: -1
    signal hovered(int index)
    signal picked(int index)

    readonly property var geom: {
        let w = 0, kw = 0, h = 6;
        for (const it of menu.items) {
            if (it.sep) { h += 9; continue; }
            w = Math.max(w, TextLayout.sysWidth(it.l));
            if (it.key) kw = Math.max(kw, TextLayout.sysWidth(it.key));
            h += 17;
        }
        return { w: 22 + w + (kw ? 18 + kw : 0) + 20, h: h, kx: 22 + w + 18 };
    }
    readonly property var rowsY: {
        const out = []; let y = 3;
        for (const it of menu.items) { out.push(y); y += it.sep ? 9 : 17; }
        return out;
    }
    function rowItem(i) { return rows.itemAt(i); }
    width: geom.w
    height: geom.h

    // Win9x bevel.
    Rectangle { anchors.fill: parent; color: Sprites.palette.menu }
    Rectangle { x: 1; y: 1; width: parent.width - 2; height: 1; color: Sprites.palette.menuHi }
    Rectangle { x: 1; y: 1; width: 1; height: parent.height - 2; color: Sprites.palette.menuHi }
    Rectangle { x: 1; y: parent.height - 2; width: parent.width - 2; height: 1; color: Sprites.palette.menuLo }
    Rectangle { x: parent.width - 2; y: 1; width: 1; height: parent.height - 2; color: Sprites.palette.menuLo }
    Rectangle { y: parent.height - 1; width: parent.width; height: 1; color: Sprites.palette.menuDk }
    Rectangle { x: parent.width - 1; width: 1; height: parent.height; color: Sprites.palette.menuDk }

    Repeater {
        id: rows
        model: menu.items
        Item {
            id: row
            required property var modelData
            required property int index
            readonly property bool hot: menu.hover === index && !modelData.dis && !modelData.sep
            readonly property string tone: modelData.dis ? "dis" : hot ? "hi" : "txt"
            x: 3; y: menu.rowsY[index] || 0
            width: menu.geom.w - 6; height: modelData.sep ? 9 : 17
            Rectangle { visible: !!row.modelData.sep; y: 3; width: parent.width; height: 1; color: Sprites.palette.menuLo }
            Rectangle { visible: !!row.modelData.sep; y: 4; width: parent.width; height: 1; color: Sprites.palette.menuHi }
            Rectangle { anchors.fill: parent; visible: row.hot; color: Sprites.palette.menuSel }
            Item {
                visible: !row.modelData.sep
                SysText { x: 20; y: 3; visible: !!row.modelData.dis; text: row.modelData.l || ""; colour: "current" }
                SysText { x: 19; y: 2; text: row.modelData.l || ""; colour: row.modelData.dis ? "grey" : row.hot ? "current" : "black" }
                SysText { x: menu.geom.kx - 2; y: 3; visible: !!row.modelData.dis && !!row.modelData.key; text: row.modelData.key || ""; colour: "current" }
                SysText { x: menu.geom.kx - 3; y: 2; visible: !!row.modelData.key; text: row.modelData.key || ""; colour: row.modelData.dis ? "grey" : row.hot ? "current" : "black" }
                SkinSprite { x: 5; y: 5; visible: !!row.modelData.check; name: "m-check-" + row.tone }
                SkinSprite { x: 6; y: 7; visible: !!row.modelData.radio; name: "m-radio-" + row.tone }
                SkinSprite { x: menu.geom.w - 15; y: 5; visible: !!row.modelData.sub; name: "m-sub-" + row.tone }
            }
            MouseArea {
                anchors.fill: parent
                hoverEnabled: true
                enabled: !row.modelData.sep
                onEntered: menu.hovered(row.index)
                // Picks on press: in a layer surface's popup under Hyprland 0.56 the
                // release of a virtual pointer can reach Qt late or not at all.
                onPressed: menu.picked(row.index)
            }
        }
    }
    // Clicks on the frame stay in the menu.
    MouseArea { z: -1; anchors.fill: parent }
}
