import QtQuick
import "lib/sprites.js" as Sprites
import "lib/text.js" as TextLayout
import "lib/tokens.js" as Tokens

// The playlist section (275 x 116 + 29n sp). In step 1 there is no source, so
// the queue, the library view (L, LIB, eject) and the jump box (J) all say so;
// item info (I, Alt+3) shows the followed item's actual metadata.
Item {
    id: root
    required property var face
    property bool active: true
    property int steps: 3
    readonly property string view: face.view
    readonly property int lh: height - 58
    readonly property int by: height - 38
    width: 275
    height: 116 + 29 * steps

    readonly property string title: ({ queue: "queue", library: "library", jump: "jump-to-item", info: "item-info" })[view]

    // Rows: { l, r, colour, rcolour, bg, center, key }. head and foot are rows too.
    readonly property var model: {
        switch (root.view) {
        case "library":
            return { head: { l: "Library", colour: "normal", bg: Sprites.palette.plHead }, rows: [{ l: "" }, { l: "No sources yet.", colour: "dim", center: true }] };
        case "jump":
            return {
                head: { l: TextLayout.sysFitLeft("Jump: ", root.face.jumpText + "_", 160), r: "no sources", colour: "current", bg: Tokens.promptBg },
                rows: [root.face.jumpText ? { l: "Nothing found for \"" + root.face.jumpText + "\": no sources yet.", colour: "dim" } : { l: "Type to search. There are no sources yet.", colour: "dim" }],
                foot: { l: "Esc: close", colour: "dim", bg: Tokens.footBg },
            };
        case "info": {
            const i = root.face.info;
            return {
                head: { l: "Item info", r: "Esc: close", colour: "normal", bg: Sprites.palette.plHead },
                rows: i.rows.length ? i.rows.map((kv) => ({ key: kv[0], l: kv[1], colour: "normal" })) : [{ l: i.empty, colour: "dim" }],
            };
        }
        }
        return { rows: [{ l: "" }, { l: "No sources yet, so nothing is queued.", colour: "dim", center: true }, { l: "Pyxamp follows the player that", colour: "dim", center: true }, { l: "started playing last.", colour: "dim", center: true }] };
    }
    property int scroll: 0
    readonly property int rowsTop: 20 + (model.head ? 13 : 0)
    readonly property int rowsH: lh - (model.head ? 13 : 0) - (model.foot ? 13 : 0)
    readonly property int fullRows: Math.max(0, Math.floor(rowsH / 13))
    readonly property int maxScroll: Math.max(0, model.rows.length - fullRows)
    onViewChanged: root.scroll = 0

    SkinSprite { name: "pl-bg-" + Math.min(root.steps, Sprites.plStepsMax) }
    SkinSprite { x: 1; y: 3; name: "pl-title-" + root.title + (root.active ? "-on" : "-off") }
    TitleDrag { x: 6; y: 3; width: 236; height: 14; face: root.face }
    TitleButton { x: 243; y: 6; kind: "album"; tip: "Album mode: needs a source" }
    TitleButton { x: 253; y: 6; kind: "shade"; tip: "Fold the playlist (Alt+E)"; onActivated: root.face.prefs.set({ plOpen: false }) }
    TitleButton { x: 263; y: 6; kind: "close"; tip: "Fold the playlist (Alt+E)"; onActivated: root.face.prefs.set({ plOpen: false }) }

    component ListRow: Item {
        id: row
        required property var r
        width: 243; height: 13
        Rectangle { anchors.fill: parent; color: row.r.bg || "transparent"; visible: !!row.r.bg }
        SysText {
            visible: !!row.r.key
            x: 2; text: row.r.key || ""; colour: "dim"
        }
        readonly property int lx: row.r.key ? 62 : 2
        readonly property int rw: row.r.r ? TextLayout.sysWidth(row.r.r) : 0
        SysText {
            readonly property string fitted: TextLayout.sysFit(row.r.l || "", 243 - 3 - row.lx - (row.rw ? row.rw + 8 : 0))
            x: row.r.center ? Math.floor((243 - TextLayout.sysWidth(fitted)) / 2) : row.lx
            text: fitted
            colour: row.r.colour || "normal"
        }
        SysText { visible: row.rw > 0; x: 243 - 3 - row.rw; text: row.r.r || ""; colour: row.r.rcolour || row.r.colour || "normal" }
    }

    // The list.
    Item {
        x: 12; y: 20; width: 243; height: root.lh
        clip: true
        ListRow { visible: !!root.model.head; r: root.model.head || ({}) }
        Item {
            y: root.rowsTop - 20; width: 243; height: root.rowsH
            clip: true
            Repeater {
                model: root.model.rows.slice(root.scroll, root.scroll + root.fullRows + 1)
                ListRow { required property var modelData; required property int index; y: index * 13; r: modelData }
            }
        }
        ListRow { visible: !!root.model.foot; y: root.lh - 13; r: root.model.foot || ({}) }
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.NoButton
            onWheel: (wheel) => { root.scroll = Math.max(0, Math.min(root.maxScroll, root.scroll + (wheel.angleDelta.y < 0 ? 1 : -1))); }
        }
    }
    Hotspot {
        x: 12; y: 20; width: 243; height: 13
        visible: root.view === "library" || root.view === "info"
        tip: root.view === "library" ? "Back to the queue (Esc)" : "Close item info (Esc)"
        onActivated: root.face.view = "queue"
    }

    // Scroll thumb.
    SkinSprite { x: 260; y: 20 + (root.maxScroll ? Math.round(root.scroll / root.maxScroll * (root.lh - 18)) : 0); name: "sthumb" }

    // Bottom bar.
    SkinButton { x: 14; y: root.by + 8; face: "btn-add"; latched: root.face.menu === "add"; tip: "Add to the queue"; onActivated: root.face.openMenu("add", this) }
    SkinButton { x: 43; y: root.by + 8; face: "btn-rem"; latched: root.face.menu === "rem"; tip: "Remove from the queue"; onActivated: root.face.openMenu("rem", this) }
    SkinButton { x: 72; y: root.by + 8; face: "btn-sel"; latched: root.face.menu === "sel"; tip: "Select"; onActivated: root.face.openMenu("sel", this) }
    SkinButton { x: 101; y: root.by + 8; face: "btn-misc"; latched: root.face.menu === "misc"; tip: "Item info, sort"; onActivated: root.face.openMenu("misc", this) }
    Text5 { x: 133; y: root.by + 7; text: "0:00/0:00"; colour: "green" }
    Repeater {
        model: ["prev", "play", "pause", "stop", "next", "eject"]
        Hotspot {
            required property string modelData
            required property int index
            readonly property var ctl: modelData === "eject" ? { enabled: true, reason: "" } : root.face.m.controls[modelData === "pause" && root.face.link.player && root.face.link.player.transport === "paused" ? "play" : modelData]
            x: 132 + index * 8; y: root.by + 18; width: 8; height: 9
            active: ctl.enabled
            tip: modelData === "eject" ? "Library (L)" : active ? modelData[0].toUpperCase() + modelData.slice(1) : ctl.reason
            onBegan: root.face.beginGesture()
            onActivated: modelData === "eject" ? root.face.showView("library") : root.face.transport(modelData, root.face.gestureTarget)
            SkinSprite { x: 1; y: 2; name: "mini-" + parent.modelData + (parent.active ? "-n" : "-d") }
        }
    }
    Text5 { x: 187; y: root.by + 20; text: root.face.m.shortClock.slice(-6).padStart(6, " "); colour: "green" }
    SkinButton { x: 222; y: root.by + 8; face: "btn-lib-" + (root.view === "library" ? 1 : 0); tip: "Queue or library (L)"; onActivated: root.face.showView("library") }
}
