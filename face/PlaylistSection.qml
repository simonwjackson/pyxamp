import QtQuick
import "lib/sprites.js" as Sprites
import "lib/text.js" as TextLayout
import "lib/tokens.js" as Tokens
import "lib/playlist.js" as Playlist
import "lib/drag.js" as Drag

// The playlist section (275 x 116 + 29n sp). It draws face.pl (lib/playlist.js):
// the active source's queue (track or album mode), the library view, the jump
// box, item info, and the prompt line over them. Rows are W95FA Grid, 13 sp.
Item {
    id: root
    required property var face
    property bool active: true
    property int steps: 3
    readonly property var model: face.pl
    readonly property int lh: height - 58
    readonly property int by: height - 38
    width: 275
    height: 116 + 29 * steps

    readonly property int rowsTop: 20 + (model.head ? 13 : 0)
    readonly property int rowsH: lh - (model.head ? 13 : 0) - (model.foot ? 13 : 0)
    readonly property int fullRows: Math.max(1, Math.floor(rowsH / 13))
    readonly property int maxScroll: Math.max(0, model.rows.length - fullRows)

    // Scroll per view (and per library level or opened album), so going back finds its
    // place. It is kept in Face.qml, so a face switch keeps it too.
    readonly property string viewKey: "winamp|" + face.view + "|" + face.lib.path.map((e) => e.source + ":" + (e.node || "")).join("/") + "|" + face.albumOpen + "|" + face.p.albumMode
    readonly property var scrolls: face.plScrolls
    readonly property int scroll: Math.max(0, Math.min(root.maxScroll, root.scrolls[root.viewKey] || 0))
    function setScroll(v) { root.face.setPlScroll(root.viewKey, Math.max(0, Math.min(root.maxScroll, v))); }
    // Keyboard selection stays in view.
    readonly property int selRow: model.noSel ? -1 : model.sel
    onSelRowChanged: {
        if (root.selRow < 0) return;
        if (root.selRow < root.scroll) root.setScroll(root.selRow);
        else if (root.selRow > root.scroll + root.fullRows - 1) root.setScroll(root.selRow - root.fullRows + 1);
    }
    // The current queue item comes into view when the queue first shows.
    Component.onCompleted: { root.revealCurrent(); root.face.rowLocator = root.rowY; }
    Component.onDestruction: if (root.face.rowLocator === root.rowY) root.face.rowLocator = null
    // Row i's middle in surface sp (describe plRowY), or -1 when it is not in view.
    function rowY(i) {
        if (!root.visible || i < root.scroll || i >= root.scroll + root.fullRows) return -1;
        return root.y + root.rowsTop + (i - root.scroll) * 13 + 6;
    }
    function revealCurrent() {
        if (root.face.view !== "queue" || root.scrolls[root.viewKey] !== undefined) return;
        const i = root.model.rows.findIndex((r) => r.colour === "current" && r.ref);
        if (i >= 0 && i >= root.fullRows) root.setScroll(i - Math.floor(root.fullRows / 2));
    }
    onViewKeyChanged: root.revealCurrent()
    Connections { target: root.face.link; function onSnapChanged() { root.revealCurrent(); } }

    // A page with more items loads its next page when the "More" row comes into view.
    readonly property bool moreVisible: {
        const r = root.model.rows;
        for (let i = root.scroll; i < Math.min(r.length, root.scroll + root.fullRows + 1); i++) if (r[i].ref && r[i].ref.type === "more") return true;
        return false;
    }
    onMoreVisibleChanged: if (root.moreVisible && root.face.view === "library") Qt.callLater(root.face.libMore)

    SkinSprite { name: "pl-bg-" + Math.min(root.steps, Sprites.plStepsMax) }
    SkinSprite { x: 1; y: 3; name: Playlist.titleSprite(root.model.title) + (root.active ? "-on" : "-off") }
    TitleDrag { x: 6; y: 3; width: 236; height: 14; face: root.face }
    TitleButton {
        x: 243; y: 6; kind: "album"; latched: root.face.p.albumMode
        tip: root.face.p.albumMode ? "Album mode on: one row per album (G)" : "Album mode: one row per album (G)"
        onActivated: root.face.toggleAlbumMode()
    }
    TitleButton { x: 253; y: 6; kind: "shade"; tip: "Fold the playlist (Alt+E)"; onActivated: root.face.prefs.set({ plOpen: false }) }
    TitleButton { x: 263; y: 6; kind: "close"; tip: "Fold the playlist (Alt+E)"; onActivated: root.face.prefs.set({ plOpen: false }) }

    component ListRow: Item {
        id: row
        required property var r
        property bool selected: false
        width: 243; height: 13
        readonly property int x0: 2 + (r.play ? 6 : 0)
        readonly property int lx: r.key ? 62 : x0 + (r.tag ? 21 : 0) + (r.tick ? 9 : 0)
        readonly property int rw: r.r ? TextLayout.sysWidth(r.r) : 0
        readonly property int mpvW: r.video ? TextLayout.sysWidth("mpv") + 4 : 0
        readonly property var chipWs: (r.chips || []).map((c) => TextLayout.sysWidth(c) + 5)
        readonly property int chipW: chipWs.reduce((a, w) => a + w + 4, 0)
        readonly property int maxW: 243 - 3 - lx - (rw ? rw + 8 : 0) - (mpvW ? mpvW + 5 : 0) - chipW
        readonly property string lt: r.alt && TextLayout.sysWidth(r.l || "") > maxW ? r.alt : (r.l || "")
        Rectangle { anchors.fill: parent; color: row.selected ? Sprites.palette.plSel : (row.r.bg || "transparent"); visible: row.selected || !!row.r.bg }
        Rectangle { visible: !!row.r.divTop; x: 2; width: 239; height: 1; color: Sprites.palette.plHead }
        SkinSprite { visible: !!row.r.play; x: 2; y: 4; name: "pl-play-" + (row.r.colour === "current" ? "current" : "normal") }
        SysText { visible: !!row.r.key; x: 2; text: row.r.key || ""; colour: "dim" }
        Item {
            visible: !!row.r.tag
            x: row.x0
            Rectangle { y: 2; width: 17; height: 9; color: row.r.tagErr ? "#4a0e08" : "#0b3a0b" }
            Text5 { x: 2; y: 4; text: row.r.tag || ""; colour: row.r.tagErr ? "err" : "green" }
        }
        SkinSprite { visible: !!row.r.tick; x: row.x0 + (row.r.tag ? 21 : 0); y: 4; name: "pl-tick" }
        SysText {
            readonly property string fitted: row.r.center ? (row.r.l || "")
                : row.r.fitLeft ? TextLayout.sysFitLeft(row.r.fitLeft, row.lt.slice(row.r.fitLeft.length), row.maxW)
                : TextLayout.sysFit(row.lt, row.maxW)
            x: row.r.center ? Math.floor((243 - TextLayout.sysWidth(fitted)) / 2) : row.lx
            text: fitted
            colour: row.r.colour || "normal"
        }
        Item {
            visible: !!row.r.video
            x: 240 - row.mpvW
            Rectangle { x: 1; y: 1; width: row.mpvW - 1; height: 11; color: row.selected ? "#4a5ad8" : "#1f3a1f" }
            SysText { x: 3; text: "mpv"; colour: row.selected ? "current" : "normal" }
        }
        SysText { visible: row.rw > 0; x: 240 - (row.mpvW ? row.mpvW + 5 : 0) - row.rw; text: row.r.r || ""; colour: row.r.rcolour || row.r.colour || "normal" }
        // A position conflict: one box per device; Left/Right picks, Enter or a click keeps.
        Repeater {
            model: row.r.chips || []
            Item {
                id: chip
                required property string modelData
                required property int index
                readonly property bool on: index === row.r.pick
                x: 240 - row.chipW + 4 + row.chipWs.slice(0, index).reduce((a, w) => a + w + 4, 0)
                y: 1; width: row.chipWs[index]; height: 11
                Rectangle { anchors.fill: parent; color: chip.on ? Sprites.palette.plSel : "#000000"; border.width: 1; border.color: chip.on ? "#8090ff" : Sprites.palette.amberMid }
                SysText { x: 3; y: -1; text: chip.modelData; colour: chip.on ? "current" : "amber" }
                Hotspot {
                    anchors.fill: parent
                    tip: "Keep this position (Enter keeps the picked one)"
                    onActivated: root.face.keepConflict(row.r.ref.conflict, chip.index)
                }
            }
        }
    }

    // The list.
    Item {
        id: list
        x: 12; y: 20; width: 243; height: root.lh
        clip: true
        ListRow { visible: !!root.model.head; r: root.model.head || ({}) }
        Item {
            y: root.rowsTop - 20; width: 243; height: root.rowsH
            clip: true
            Repeater {
                model: root.model.rows.slice(root.scroll, root.scroll + root.fullRows + 1)
                ListRow {
                    id: lr
                    required property var modelData
                    required property int index
                    readonly property int at: root.scroll + index
                    y: index * 13; r: modelData
                    selected: !root.model.noSel && root.model.sel === at && !!modelData.ref && !modelData.group && !modelData.chips
                    Hotspot {
                        anchors.fill: parent
                        z: -1
                        visible: !root.model.inert
                        // A cut row keeps its full text one hover away.
                        tip: lr.modelData.tip || (!lr.modelData.center && TextLayout.sysWidth(lr.modelData.l || "") > lr.maxW ? lr.modelData.l + (lr.modelData.r ? "  (" + lr.modelData.r + ")" : "") : "")
                        onActivated: root.face.setSel(lr.at)
                        onDoubleClicked: (mouse) => { if (mouse.button === Qt.LeftButton) root.face.rowEnter(lr.at, false); }
                        onSecondary: (mx, my) => { menuPoint.x = list.x + mx; menuPoint.y = list.y + lr.y + (root.rowsTop - 20) + my; root.face.openRowMenu(lr.at, menuPoint); }
                    }
                }
            }
        }
        ListRow { visible: !!root.model.foot; y: root.lh - 13; r: root.model.foot || ({}) }
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.NoButton
            onWheel: (wheel) => root.setScroll(root.scroll + (wheel.angleDelta.y < 0 ? 3 : -3))
        }
    }
    Item { id: menuPoint; width: 1; height: 0 }
    // The head row goes back: up a library level, out of an album, out of item info.
    Hotspot {
        x: 12; y: 20; width: 243; height: 13
        visible: !!root.model.head && !!root.model.head.ref && !root.face.prompt
        tip: !root.model.head || !root.model.head.ref ? "" : root.model.head.ref.type === "libBack" ? "Back (Backspace)" : root.model.head.ref.type === "albumBack" ? "Back to the albums (Esc)" : "Back to the queue (Esc)"
        onActivated: {
            const t = root.model.head.ref.type;
            if (t === "libBack") root.face.libBack(); else if (t === "albumBack") root.face.albumBack(); else root.face.view = "queue";
        }
    }

    // Scroll groove and thumb: a drag across the groove is the whole list.
    SkinSprite { x: 260; y: groove.y + Drag.thumbOffset(root.maxScroll ? root.scroll / root.maxScroll : 0, groove.height, groove.grip); name: groove.pressed ? "sthumb-p" : "sthumb" }
    Hotspot {
        id: groove
        x: 259; y: 20; width: 10; height: root.lh
        tip: root.model.rows.length > root.fullRows ? root.model.rows.length.toLocaleString(Qt.locale("en_US"), "f", 0) + " rows" : ""
        readonly property int grip: 18          // the thumb
        function to(my) { root.setScroll(Math.round(Drag.fraction(my, height, grip) * root.maxScroll)); }
        onBegan: to(mouseY)
        onPositionChanged: (mouse) => { if (pressed) to(mouse.y); }
    }

    // Bottom bar.
    SkinButton { x: 14; y: root.by + 8; face: "btn-add"; latched: root.face.menu === "add"; tip: "Add to the queue"; onActivated: root.face.openMenu("add", this) }
    SkinButton { x: 43; y: root.by + 8; face: "btn-rem"; latched: root.face.menu === "rem"; tip: "Remove from the queue"; onActivated: root.face.openMenu("rem", this) }
    SkinButton { x: 72; y: root.by + 8; face: "btn-sel"; latched: root.face.menu === "sel"; tip: "Select"; onActivated: root.face.openMenu("sel", this) }
    SkinButton { x: 101; y: root.by + 8; face: "btn-misc"; latched: root.face.menu === "misc"; tip: "Item info, mark as played, album mode, save as playlist"; onActivated: root.face.openMenu("misc", this) }
    Text5 { x: 133; y: root.by + 7; text: Playlist.totals(root.face.plCtx).slice(0, 17); colour: "green" }
    Repeater {
        model: ["prev", "play", "pause", "stop", "next", "eject"]
        Hotspot {
            required property string modelData
            required property int index
            readonly property var ctl: modelData === "eject" ? { enabled: true, reason: "" } : root.face.m.controls[modelData === "pause" && root.face.m.paused ? "play" : modelData]
            x: 132 + index * 8; y: root.by + 18; width: 8; height: 9
            active: ctl.enabled
            tip: modelData === "eject" ? "Library (L)" : active ? modelData[0].toUpperCase() + modelData.slice(1) : ctl.reason
            onBegan: root.face.beginGesture()
            onActivated: modelData === "eject" ? root.face.showView("library") : root.face.transport(modelData, root.face.gesture)
            SkinSprite { x: 1; y: 2; name: "mini-" + parent.modelData + (parent.active ? "-n" : "-d") }
        }
    }
    Text5 { x: 187; y: root.by + 20; text: root.face.m.shortClock.slice(-6).padStart(6, " "); colour: "green" }
    SkinButton { x: 222; y: root.by + 8; face: "btn-lib-" + (root.face.view === "library" ? 1 : 0); tip: "Queue or library (L)"; onActivated: root.face.showView("library") }
}
