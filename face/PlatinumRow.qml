import QtQuick
import "lib/platinum.js" as Pt

// One 16-sp row of the Platinum face's list view, drawn from a model row
// (lib/playlist.js; lib/outline.js for the library). kind: queue | album |
// library | jump | info. Columns are list-relative [x, w] pairs.
Item {
    id: root
    required property var face
    required property var entry
    property string kind: "queue"
    property var colX: []
    property int listW: 271
    property bool following: false
    property bool selected: false
    property bool inert: false
    signal picked(bool dbl)
    signal triangle()
    signal menu(real mx, real my)
    readonly property var pa: face.pal
    readonly property var r: entry.row
    readonly property bool canSelect: !!r.ref && !r.group && !inert
    width: listW
    height: 16

    function tone(c) { return c === "dim" ? root.pa.off : c === "err" ? root.pa.danger : root.pa.text; }
    readonly property color tc: root.selected ? root.pa.selText : root.tone(root.r.colour)
    readonly property color t2: root.selected ? root.pa.selText : root.r.rcolour === "err" ? root.pa.danger : root.r.colour === "dim" ? root.pa.off : root.r.colour === "err" ? root.pa.danger : root.pa.text2
    readonly property bool current: root.r.colour === "current" || !!root.r.play
    readonly property var split: Pt.splitNumber(root.r.l)

    Rectangle { anchors.fill: parent; visible: root.selected; color: root.pa.sel }
    Rectangle { visible: !!root.r.divTop; width: root.listW; height: 1; color: root.pa.ridge }
    // A position conflict waits on this row.
    Rectangle { visible: !!root.r.chips && !root.selected; anchors.fill: parent; color: root.pa.well }

    // ---------- queue and album rows: #, Name, Time ----------
    Item {
        visible: (root.kind === "queue" || root.kind === "album") && !root.r.center
        readonly property var c0: root.colX[0] || [0, 30]
        readonly property var c1: root.colX[1] || [30, 190]
        readonly property var c2: root.colX[2] || [220, 51]
        PlatinumRects {
            visible: root.current
            rects: { const o = []; Pt.glyph(o, root.following && root.r.colour === "current" ? "pausemini" : "speaker", parent.c0[0] + 2, 4, root.selected ? root.pa.selText : root.pa.text); return o; }
        }
        PlatinumText {
            face: root.face; size: 11; text: root.split.n; color: root.selected ? root.pa.selText : root.pa.text2; ty: 2
            x: parent.c0[0] + parent.c0[1] - 5 - implicitWidth
        }
        PlatinumRects {
            visible: !!root.r.tick
            rects: { const o = []; Pt.glyph(o, "check", parent.c1[0] + 5, 5, root.tc); return o; }
        }
        PlatinumText {
            readonly property int nx: parent.c1[0] + 5 + (root.r.tick ? 10 : 0)
            face: root.face; text: root.split.name; color: root.tc; ty: 2; x: nx
            width: Math.max(0, parent.c1[0] + parent.c1[1] - nx - 4 - chips.width); elide: Text.ElideRight
        }
        PlatinumText {
            visible: !root.r.chips
            face: root.face; size: 11; text: root.r.r || ""; ty: 2
            color: root.current && !root.following ? root.tc : root.t2
            x: parent.c2[0] + parent.c2[1] - 6 - implicitWidth
        }
        // Conflict: one small button per position; Left and Right pick, Enter keeps.
        Row {
            id: chips
            visible: !!root.r.chips
            x: root.listW - 4 - width; y: 1
            spacing: 3
            Repeater {
                model: root.r.chips || []
                PlatinumButton {
                    id: chip
                    required property string modelData
                    required property int index
                    pal: root.pa
                    width: Math.ceil(chipText.implicitWidth) + 10; height: 14
                    on: index === root.r.pick
                    tip: "Keep this position (Enter keeps the picked one)"
                    onActivated: root.face.keepConflict(root.r.ref.conflict, chip.index)
                    PlatinumText { id: chipText; face: root.face; size: 10; text: chip.modelData; x: 5 + chip.o; ty: 1 + chip.o; color: chip.on ? root.pa.sel : root.pa.text }
                }
            }
        }
    }
    // A centred line (an empty queue's invitation).
    PlatinumText {
        visible: !!root.r.center
        face: root.face; text: root.r.l || ""; color: root.pa.text2; ty: 2
        x: Math.round(root.listW / 2 - implicitWidth / 2)
    }

    // ---------- the library outline: triangle, indent, Name, Items ----------
    Item {
        visible: root.kind === "library" && !root.r.center
        readonly property var c0: root.colX[0] || [0, 201]
        readonly property var c1: root.colX[1] || [201, 70]
        readonly property int ix: c0[0] + 4 + root.entry.depth * 14
        readonly property int nx: ix + 15
        readonly property int badge: root.r.video ? Math.ceil(mpv.implicitWidth) + 8 : 0
        PlatinumRects {
            visible: !!root.entry.tri
            rects: { const o = []; Pt.discl(o, root.pa, parent.ix, 3, root.entry.tri === "open", root.selected ? root.pa.selText : root.pa.text); return o; }
        }
        PlatinumText {
            id: libName
            face: root.face; text: root.r.l || ""; ty: 2; x: parent.nx
            color: root.current && !root.selected ? root.pa.text : root.tc
            width: Math.min(implicitWidth, Math.max(0, parent.c0[0] + parent.c0[1] - parent.nx - 6 - parent.badge)); elide: Text.ElideRight
        }
        PlatinumRects {
            visible: root.current && !root.selected
            rects: { const o = []; Pt.glyph(o, "speaker", parent.nx + Math.ceil(libName.width) + 4, 4, root.pa.text); return o; }
        }
        Rectangle {
            visible: !!root.r.video
            x: parent.c0[0] + parent.c0[1] - parent.badge - 2; y: 3; width: parent.badge; height: 11
            color: root.selected ? root.pa.selText : root.pa.well
            PlatinumText { id: mpv; face: root.face; size: 10; text: "mpv"; color: root.selected ? root.pa.sel : root.pa.text; x: 4; ty: 0 }
        }
        PlatinumText {
            face: root.face; size: 11; text: root.r.r || ""; ty: 2
            color: root.selected ? root.pa.selText : root.r.rcolour === "err" ? root.pa.danger : root.pa.text2
            x: parent.c1[0] + parent.c1[1] - 6 - implicitWidth
        }
        Hotspot {
            visible: !!root.entry.tri && !root.inert
            x: parent.ix - 2; y: 0; width: 15; height: 16
            tip: root.entry.tri === "open" ? "Fold (Left)" : "Unfold (Right)"
            onActivated: root.triangle()
        }
    }

    // ---------- jump results: Name, Where (a label chip and the source), Time ----------
    Item {
        visible: root.kind === "jump"
        readonly property var c0: root.colX[0] || [0, 167]
        readonly property var c1: root.colX[1] || [167, 60]
        readonly property var c2: root.colX[2] || [227, 44]
        readonly property string src: Pt.TAG_SOURCE[root.r.tag] || ""
        readonly property color chipCol: root.r.tagErr ? root.pa.danger : (root.pa.labels[src] || root.pa.off)
        PlatinumText {
            face: root.face; size: root.r.group ? 11 : 12; text: root.r.l || ""; ty: 2
            color: root.r.group ? (root.r.colour === "err" ? root.pa.danger : root.pa.text2) : root.tc
            x: root.r.group ? 6 : parent.c0[0] + 5
            width: Math.min(implicitWidth, root.r.group ? (root.r.tag ? parent.c0[1] : root.listW) - 12 : parent.c0[1] - 10 - (root.r.video ? 30 : 0)); elide: Text.ElideRight
        }
        Rectangle {
            visible: !!root.r.video && !root.r.group
            x: parent.c0[0] + parent.c0[1] - 30; y: 3; width: 26; height: 11
            color: root.selected ? root.pa.selText : root.pa.well
            PlatinumText { face: root.face; size: 10; text: "mpv"; color: root.selected ? root.pa.sel : root.pa.text; x: 4; ty: 0 }
        }
        Rectangle {
            visible: !!root.r.tag
            x: parent.c1[0] + 5; y: 4; width: 7; height: 7
            color: parent.chipCol
            border.width: 1
            border.color: root.pa.flat ? "#000000" : Pt.mix(String(parent.chipCol), "#000000", 0.35)
            antialiasing: false
        }
        PlatinumText {
            visible: !!root.r.tag
            face: root.face; size: 10; text: Pt.SOURCE_NAME[root.r.tag] || root.r.tag || ""; ty: 3; x: parent.c1[0] + 14
            color: root.selected ? root.pa.selText : root.pa.text2
        }
        PlatinumText {
            visible: !root.r.group
            face: root.face; size: 11; text: root.r.r || ""; ty: 2; color: root.t2
            x: parent.c2[0] + parent.c2[1] - 6 - implicitWidth
        }
    }

    // ---------- item info: key, value ----------
    property int keyW: 70
    Item {
        visible: root.kind === "info"
        PlatinumText { face: root.face; size: 11; text: root.r.key || ""; color: root.pa.text2; ty: 2; x: 6 }
        PlatinumText { face: root.face; size: 11; text: root.r.l || ""; color: root.tone(root.r.colour); ty: 2; x: root.r.key ? 6 + root.keyW : 6; width: Math.min(implicitWidth, root.listW - 16 - root.keyW); elide: Text.ElideRight }
    }

    // The pointer: a click selects, a double click is Enter, a right click the row's menu.
    Hotspot {
        anchors.fill: parent
        z: -1
        visible: !root.inert && !!root.r.ref
        tip: root.r.tip || (root.r.video ? "Opens in mpv, outside Pyxamp" : "")
        onActivated: root.picked(false)
        onDoubleClicked: (mouse) => { if (mouse.button === Qt.LeftButton) root.picked(true); }
        onSecondary: (mx, my) => root.menu(mx, my)
    }
}
