import QtQuick
import Quickshell

// A popup menu: an xdg popup of the Pyxamp surface, at the surface's scale,
// next to the button that opened it. It may extend past the column (the brief
// allows menus and tooltips outside the surface). It grabs the keyboard while
// open, so keys never leak to transport; a click elsewhere dismisses it.
// A submenu opens beside its row inside the same popup, which grows for it.
// platinum: the Platinum face's menus (PlatinumMenuPanel.qml), drawn at the
// monitor's own scale (scaleFactor 1); the keys and picks are the same.
PopupWindow {
    id: menu
    required property var face
    property bool platinum: false
    property string kind: ""
    property Item anchorItem: null
    property real scaleFactor: 1
    signal dismissed()

    readonly property bool up: ["add", "rem", "sel", "misc"].indexOf(kind) >= 0
    readonly property var items: kind ? face.menuItems(kind) : []
    property int hover: -1
    property int subIndex: -1
    property int subHover: -1
    readonly property var subItems: subIndex >= 0 && items[subIndex] && items[subIndex].sub ? face.menuItems(items[subIndex].sub) : []

    function selectable(list, i) { const it = list[i]; return !!it && !it.sep && !it.dis; }
    function step(list, from, dir) {
        let i = from;
        for (let k = 0; k < list.length; k++) { i = (i + dir + list.length) % list.length; if (menu.selectable(list, i)) return i; }
        return from;
    }
    function hoverMain(i) { menu.hover = i; const it = menu.items[i]; menu.subIndex = it && it.sub && !it.dis ? i : -1; menu.subHover = -1; }
    function pick(list, i, inSub) {
        const it = list[i];
        if (!it || it.sep || it.dis) return;
        if (it.sub && !inSub) { menu.subIndex = i; menu.subHover = -1; return; }
        menu.face.menu = "";
        if (it.act) it.act();
    }

    readonly property var panel: menu.platinum ? pPanel : wPanel
    readonly property var subPanel: menu.platinum ? pSubPanel : wSubPanel
    readonly property real subX: menu.platinum ? panel.width - 4 - 2 * pPanel.hair : panel.width - 3
    readonly property real subY: menu.subIndex >= 0 ? (panel.rowsY[menu.subIndex] || 0) - (menu.platinum ? 4 + pPanel.hair : 3) : 0
    readonly property real spW: menu.subItems.length ? subX + subPanel.width : panel.width
    readonly property real spH: menu.subItems.length ? Math.max(panel.height, subY + subPanel.height) : panel.height

    color: "transparent"
    grabFocus: true
    anchor.item: menu.anchorItem
    anchor.rect.x: 0
    anchor.rect.y: menu.up ? 0 : (menu.anchorItem ? menu.anchorItem.height : 0)
    anchor.edges: menu.up ? Edges.Top | Edges.Left : Edges.Bottom | Edges.Left
    anchor.gravity: menu.up ? Edges.Top | Edges.Right : Edges.Bottom | Edges.Right
    implicitWidth: Math.ceil(spW * scaleFactor)
    implicitHeight: Math.ceil(spH * scaleFactor)

    onKindChanged: { menu.hover = -1; menu.subIndex = -1; menu.subHover = -1; menu.visible = menu.kind !== "" && menu.anchorItem !== null; }
    onVisibleChanged: if (!menu.visible && menu.kind !== "") menu.dismissed()

    Item {
        id: body
        width: menu.spW
        height: menu.spH
        transformOrigin: Item.TopLeft
        scale: menu.scaleFactor
        focus: true
        Keys.onPressed: (e) => {
            const inSub = menu.subIndex >= 0 && menu.subHover >= 0;
            switch (e.key) {
            case Qt.Key_Down: if (inSub) menu.subHover = menu.step(menu.subItems, menu.subHover, 1); else menu.hover = menu.step(menu.items, menu.hover, 1); break;
            case Qt.Key_Up: if (inSub) menu.subHover = menu.step(menu.subItems, menu.subHover, -1); else menu.hover = menu.step(menu.items, menu.hover, -1); break;
            case Qt.Key_Right:
                if (!inSub && menu.selectable(menu.items, menu.hover) && menu.items[menu.hover].sub) { menu.subIndex = menu.hover; menu.subHover = menu.step(menu.face.menuItems(menu.items[menu.hover].sub), -1, 1); }
                break;
            case Qt.Key_Left: if (menu.subIndex >= 0) { menu.subIndex = -1; menu.subHover = -1; } break;
            case Qt.Key_Return: case Qt.Key_Enter: case Qt.Key_Space:
                if (inSub) menu.pick(menu.subItems, menu.subHover, true); else menu.pick(menu.items, menu.hover, false);
                break;
            case Qt.Key_Escape: if (menu.subIndex >= 0) { menu.subIndex = -1; menu.subHover = -1; } else menu.visible = false; break;
            }
            e.accepted = true;
        }

        MenuPanel {
            id: wPanel
            visible: !menu.platinum
            y: menu.up ? menu.spH - height : 0
            items: menu.platinum ? [] : menu.items
            hover: menu.hover
            onHovered: (i) => menu.hoverMain(i)
            onPicked: (i) => menu.pick(menu.items, i, false)
        }
        MenuPanel {
            id: wSubPanel
            visible: !menu.platinum && menu.subItems.length > 0
            x: menu.subX
            y: menu.subY
            items: menu.platinum ? [] : menu.subItems
            hover: menu.subHover
            onHovered: (i) => menu.subHover = i
            onPicked: (i) => menu.pick(menu.subItems, i, true)
        }
        PlatinumMenuPanel {
            id: pPanel
            face: menu.face
            visible: menu.platinum
            y: menu.up ? menu.spH - height : 0
            items: menu.platinum ? menu.items : []
            hover: menu.hover
            onHovered: (i) => menu.hoverMain(i)
            onPicked: (i) => menu.pick(menu.items, i, false)
        }
        PlatinumMenuPanel {
            id: pSubPanel
            face: menu.face
            visible: menu.platinum && menu.subItems.length > 0
            x: menu.subX
            y: menu.subY
            items: menu.platinum ? menu.subItems : []
            hover: menu.subHover
            onHovered: (i) => menu.subHover = i
            onPicked: (i) => menu.pick(menu.subItems, i, true)
        }
    }
}
