import QtQuick
import Quickshell
import "lib/sprites.js" as Sprites
import "lib/text.js" as TextLayout

// A Win9x-grey popup menu in W95FA Grid at the surface's scale (decision 15).
// It is an xdg popup of the Pyxamp surface, so it may extend past the column,
// and it holds the keyboard while open: keys never leak to the surface.
// items: [{ l, key?, act?, dis?, check?, radio?, sub?, sep? }] from Face.menuItems.
PopupWindow {
    id: menu
    required property var face
    property var host: null
    property string kind: ""
    property Item anchorItem: null
    property real scaleFactor: 1
    property bool submenu: false
    signal dismissed()

    readonly property bool up: ["add", "rem", "sel", "misc"].indexOf(kind) >= 0
    readonly property var items: kind ? face.menuItems(kind) : []
    property int hover: -1
    property int subIndex: -1

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

    function selectable(i) { const it = menu.items[i]; return it && !it.sep && !it.dis; }
    function move(dir) {
        let i = menu.hover;
        for (let k = 0; k < menu.items.length; k++) {
            i = (i + dir + menu.items.length) % menu.items.length;
            if (menu.selectable(i)) { menu.hover = i; return; }
        }
    }
    function activate(i) {
        const it = menu.items[i];
        if (!it || it.sep || it.dis) return;
        if (it.sub) { menu.subIndex = i; return; }
        menu.face.menu = "";
        if (it.act) it.act();
    }

    color: "transparent"
    grabFocus: true
    anchor.item: menu.anchorItem
    anchor.rect.x: menu.submenu ? (menu.anchorItem ? menu.anchorItem.width + 3 : 0) : 0
    anchor.rect.y: menu.submenu ? -3 : menu.up ? 0 : (menu.anchorItem ? menu.anchorItem.height : 0)
    anchor.edges: menu.up ? Edges.Top | Edges.Left : Edges.Bottom | Edges.Left
    anchor.gravity: menu.up ? Edges.Top | Edges.Right : Edges.Bottom | Edges.Right
    implicitWidth: Math.ceil(geom.w * scaleFactor)
    implicitHeight: Math.ceil(geom.h * scaleFactor)

    onKindChanged: { menu.hover = -1; menu.subIndex = -1; menu.visible = menu.kind !== "" && menu.anchorItem !== null; }
    onVisibleChanged: if (!menu.visible && menu.kind !== "") menu.dismissed()
    Component.onCompleted: menu.visible = menu.kind !== "" && menu.anchorItem !== null

    Item {
        id: body
        width: menu.geom.w
        height: menu.geom.h
        transformOrigin: Item.TopLeft
        scale: menu.scaleFactor
        focus: true
        Keys.onPressed: (e) => {
            switch (e.key) {
            case Qt.Key_Down: menu.move(1); break;
            case Qt.Key_Up: menu.move(-1); break;
            case Qt.Key_Return: case Qt.Key_Enter: case Qt.Key_Space: menu.activate(menu.hover); break;
            case Qt.Key_Right: if (menu.selectable(menu.hover) && menu.items[menu.hover].sub) menu.subIndex = menu.hover; break;
            case Qt.Key_Left: if (menu.submenu) menu.visible = false; break;
            case Qt.Key_Escape: menu.visible = false; break;
            }
            e.accepted = true;
        }

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
                    onEntered: { menu.hover = row.index; menu.subIndex = row.modelData.sub && !row.modelData.dis ? row.index : -1; }
                    onClicked: menu.activate(row.index)
                }
            }
        }
    }

    // The submenu: another PopupMenu, opened beside its row.
    Loader {
        id: sub
        Connections {
            target: sub.item
            function onDismissed() { menu.subIndex = -1; }
        }
    }
    onSubIndexChanged: {
        sub.source = "";
        if (menu.subIndex < 0 || !menu.visible) return;
        sub.setSource(Qt.resolvedUrl("PopupMenu.qml"), {
            face: menu.face, submenu: true, scaleFactor: menu.scaleFactor,
            anchorItem: rows.itemAt(menu.subIndex), kind: menu.items[menu.subIndex].sub,
        });
    }
}
