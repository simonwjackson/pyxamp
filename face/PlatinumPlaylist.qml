import QtQuick
import "lib/platinum.js" as Pt
import "lib/lcdcase.js" as Case
import "lib/outline.js" as Outline
import "lib/drag.js" as Drag

// The Platinum face's playlist section: a Finder list view. A 17-sp
// disclosure header that stays when folded; then a header line ("14 items,
// 4:10 of 1:09:31") or the jump box's Find field, column titles with the
// sorted column shaded, 16-sp rows, a Platinum scroll bar, the pop-up buttons
// and a grow box. It draws face.pl (lib/playlist.js), the same rows the Winamp
// face draws, except the library view, which is a Finder outline of the same
// rows (lib/outline.js). Every key, click and menu calls the same Face.qml
// action as the Winamp face.
Item {
    id: root
    required property var face
    property bool active: true
    property int steps: 3
    readonly property bool open: face.p.plOpen
    readonly property var pa: face.pal
    readonly property var model: face.pl
    readonly property int rowsN: 4 + 2 * steps
    width: 300
    height: open ? 82 + 16 * rowsN : 17

    FontMetrics { id: fm11; font.family: root.face.chicago; font.pixelSize: 11 }
    FontMetrics { id: fm12; font.family: root.face.chicago; font.pixelSize: 12 }

    // ---------- what kind of list ----------
    readonly property string kind: {
        if (root.face.view === "library") return "library";
        if (root.face.view === "jump") return "jump";
        if (root.face.view === "info") return "info";
        if (root.face.p.albumMode && !root.face.albumOpen && root.model.rows.some((r) => r.ref && r.ref.type === "album")) return "album";
        return "queue";
    }
    readonly property bool field: root.kind === "jump" || !!root.face.prompt
    readonly property var outline: root.kind === "library" ? Outline.rows(root.face.plCtx) : []
    // One entry per drawn row: { row, at (model index or -1), depth, level, idx, tri }.
    readonly property var entries: root.kind === "library" ? root.outline
        : root.model.rows.map((r, i) => ({ row: r, at: i, depth: 0, level: -1, idx: i, tri: null }))
    readonly property int selEntry: {
        if (root.model.noSel || root.model.sel < 0) return -1;
        if (root.kind === "library") return Outline.selIndex(root.outline, root.model.sel);
        const r = root.model.rows[root.model.sel];
        return r && r.ref && !r.group && !r.chips ? root.model.sel : -1;
    }
    readonly property var queue: root.face.link.snap ? root.face.link.snap.queue : []
    readonly property string strip: {
        const h = root.model.head;
        switch (root.kind) {
        case "library": return "Right unfolds, Left folds, Enter plays";
        case "info": return "Esc: close";
        case "album": return Pt.queueInfo(root.queue, root.face.sel);
        }
        if (h && h.ref && h.ref.type === "albumBack") return h.l + (h.r ? "   " + h.r : "");
        return root.queue.length ? Pt.queueInfo(root.queue, root.face.sel) : "0 items";
    }
    readonly property bool stripBack: !root.field && (root.kind === "info" || (!!root.model.head && !!root.model.head.ref && root.model.head.ref.type === "albumBack"))

    // ---------- the header (stays when folded) ----------
    Rectangle { x: 1; width: 298; height: root.height; color: root.pa.face }
    PlatinumRects { rects: Pt.header(root.pa, root.open) }
    PlatinumText { id: label; face: root.face; text: Case.title(root.model.title); x: 24; ty: 3; color: root.pa.text }
    PlatinumText {
        visible: !root.open
        face: root.face; size: 11; text: root.strip; color: root.pa.text2; ty: 3
        x: 290 - implicitWidth
    }
    Hotspot {
        x: 4; y: 1; width: Math.min(160, Math.ceil(label.implicitWidth) + 30); height: 16
        tip: root.open ? "Fold the playlist (Alt+E)" : "Unfold the playlist (Alt+E)"
        onActivated: root.face.prefs.set({ plOpen: !root.open })
    }

    // ---------- geometry ----------
    readonly property int lx: 7
    readonly property int listW: 271
    readonly property int listTop: 18
    readonly property int listBottom: root.height - 30
    readonly property int stripH: root.field ? 22 : 15
    // The field starts after its label ("Find:", or the prompt's "Name:").
    readonly property int fieldX: root.lx + 6 + Math.ceil(fm11.advanceWidth(root.face.prompt ? root.face.prompt.label + ":" : "Find:")) + 7
    readonly property int fieldW: root.listW - (root.fieldX - root.lx) - 6
    readonly property var cols: {
        let nw = 0, tw = 0;
        if (root.kind === "queue" || root.kind === "album") nw = fm11.advanceWidth(String(root.model.rows.length));
        if (root.kind === "queue" || root.kind === "album" || root.kind === "jump")
            for (const r of root.model.rows) if (r.r && !r.group) tw = Math.max(tw, fm11.advanceWidth(r.r));
        return Pt.columns(root.kind === "info" ? "info" : root.kind, Math.ceil(nw), Math.ceil(tw));
    }
    readonly property var colX: Pt.columnX(root.cols, root.lx, root.listW)
    // Item info: the keys' column fits the widest key.
    readonly property int infoKeyW: {
        if (root.kind !== "info") return 70;
        let w = 0;
        for (const r of root.model.rows) if (r.key) w = Math.max(w, fm11.advanceWidth(r.key));
        return Math.max(70, Math.ceil(w) + 10);
    }
    readonly property int colsTop: root.listTop + 1 + root.stripH + 1
    readonly property int rowsTop: root.colsTop + (root.cols.length ? 16 : 0)
    readonly property int footH: root.model.foot ? 16 : 0
    readonly property int rowsH: root.listBottom - 1 - root.rowsTop - root.footH
    readonly property int fullRows: Math.max(1, Math.floor(root.rowsH / 16))
    readonly property int maxScroll: Math.max(0, root.entries.length - root.fullRows)

    // Scroll per view (and per library path or opened album), kept in Face.qml, so a
    // face switch, a fold or a resize keeps the place.
    readonly property string viewKey: "platinum|" + root.face.view + "|" + root.face.lib.path.map((e) => e.source + ":" + (e.node || "")).join("/") + "|" + root.face.albumOpen + "|" + root.face.p.albumMode
    readonly property int scroll: Math.max(0, Math.min(root.maxScroll, root.face.plScrolls[root.viewKey] || 0))
    function setScroll(v) { root.face.setPlScroll(root.viewKey, Math.max(0, Math.min(root.maxScroll, v))); }
    onSelEntryChanged: root.reveal()
    onFullRowsChanged: root.reveal()
    function reveal() {
        if (root.selEntry < 0) return;
        if (root.selEntry < root.scroll) root.setScroll(root.selEntry);
        else if (root.selEntry > root.scroll + root.fullRows - 1) root.setScroll(root.selEntry - root.fullRows + 1);
    }
    Component.onCompleted: { root.revealCurrent(); root.face.rowLocator = root.rowY; }
    Component.onDestruction: if (root.face.rowLocator === root.rowY) root.face.rowLocator = null
    // Model row i's middle in surface sp (describe plRowY), or -1 when it is not in view.
    function rowY(i) {
        if (!root.open) return -1;
        const e = root.kind === "library" ? root.entries.findIndex((x) => x.at === i) : i;
        if (e < 0 || e < root.scroll || e >= root.scroll + root.fullRows) return -1;
        return root.y + root.rowsTop + (e - root.scroll) * 16 + 8;
    }
    onViewKeyChanged: root.revealCurrent()
    Connections { target: root.face.link; function onSnapChanged() { root.revealCurrent(); } }
    function revealCurrent() {
        if (root.face.view !== "queue" || root.face.plScrolls[root.viewKey] !== undefined) { root.reveal(); return; }
        const i = root.model.rows.findIndex((r) => r.colour === "current" && r.ref);
        if (i >= 0 && i >= root.fullRows) root.setScroll(i - Math.floor(root.fullRows / 2));
    }
    // The "More" row in view loads the next page, as on the Winamp face.
    readonly property bool moreVisible: {
        const e = root.entries;
        for (let i = root.scroll; i < Math.min(e.length, root.scroll + root.fullRows + 1); i++) if (e[i].row.ref && e[i].row.ref.type === "more") return true;
        return false;
    }
    onMoreVisibleChanged: if (root.moreVisible && root.face.view === "library") Qt.callLater(root.face.libMore)

    // Paused-free blink for the field's caret (1 s), still under reduced motion.
    property bool caretOn: true
    Timer { interval: 500; repeat: true; running: root.open && root.field && !root.face.p.reducedMotion && root.face.p.shown; onTriggered: root.caretOn = !root.caretOn; onRunningChanged: root.caretOn = true }

    Item {
        visible: root.open
        width: 300; height: root.height

        // The list frame, the header line or the Find field, the column titles.
        PlatinumRects {
            rects: {
                const o = [], A = root.pa, lx = root.lx, y = root.listTop + 1;
                Pt.R(o, lx - 1, root.listTop, root.listW + 17, root.listBottom - root.listTop, A.edge);
                Pt.R(o, lx, root.listTop + 1, root.listW, root.listBottom - root.listTop - 2, A.list);
                Pt.R(o, lx, y, root.listW, root.stripH, A.face);
                if (!root.field) { if (!A.flat) Pt.R(o, lx, y, root.listW, 1, A.hi); }
                else {
                    const fx = root.fieldX, fw = root.listW - (root.fieldX - lx) - 6;
                    Pt.R(o, fx - 2, y + 2, fw + 4, 18, A.sel);
                    Pt.R(o, fx - 1, y + 3, fw + 2, 16, A.flat ? "#000000" : A.list);
                    if (!A.flat) Pt.bevel(o, fx - 1, y + 3, fw + 2, 16, A.lo, A.hi);
                    Pt.R(o, fx, y + 4, fw, 14, A.flat ? "#000000" : A.field);
                }
                Pt.R(o, lx, y + root.stripH, root.listW, 1, A.edge);
                const cy = root.colsTop;
                for (let i = 0; i < root.cols.length && i < root.colX.length; i++) {
                    const cx = root.colX[i][0], cw = root.colX[i][1], sorted = i === 0 && (root.kind === "queue" || root.kind === "album");
                    Pt.R(o, cx, cy, cw, 15, sorted ? A.headSorted : A.raised);
                    if (!A.flat) Pt.bevel(o, cx, cy, cw, 15, A.hi, A.lo); else Pt.R(o, cx + cw - 1, cy, 1, 15, A.ridge);
                }
                if (root.cols.length) Pt.R(o, lx, cy + 15, root.listW, 1, A.edge);
                // The shaded sorted column (#), as in Finder.
                if ((root.kind === "queue" || root.kind === "album") && root.entries.length && root.colX.length) Pt.R(o, root.colX[0][0], root.rowsTop, root.colX[0][1], root.rowsH, A.sorted);
                return o;
            }
        }
        // The header line, or the field.
        PlatinumText {
            visible: !root.field
            face: root.face; size: 10; text: root.strip; color: root.pa.text2; ty: root.listTop + 3
            x: root.lx + Math.round(root.listW / 2 - implicitWidth / 2)
            width: Math.min(implicitWidth, root.listW - 8); elide: Text.ElideMiddle
        }
        Hotspot {
            visible: root.stripBack
            x: root.lx; y: root.listTop + 1; width: root.listW; height: root.stripH
            tip: root.kind === "info" ? "Back to the queue (Esc)" : "Back to the albums (Esc)"
            onActivated: { if (root.kind === "info") root.face.view = "queue"; else root.face.albumBack(); }
        }
        Item {
            visible: root.field
            readonly property var pr: root.face.prompt
            readonly property string flabel: pr ? pr.label + ":" : "Find:"
            readonly property string ftext: pr ? pr.text : root.face.jump.text
            readonly property string fright: root.model.head ? root.model.head.r || "" : ""
            PlatinumText { face: root.face; size: 11; text: parent.flabel; color: root.pa.text; x: root.lx + 6; ty: root.listTop + 6 }
            PlatinumText {
                id: ftext
                face: root.face; size: 11; text: parent.ftext; color: root.pa.text
                x: root.fieldX + 3; ty: root.listTop + 6
                width: Math.min(implicitWidth, root.fieldW - 70); elide: Text.ElideLeft
            }
            Rectangle { visible: root.caretOn; x: root.fieldX + 4 + Math.ceil(ftext.width); y: root.listTop + 7; width: 1; height: 11; color: root.pa.text }
            PlatinumText { face: root.face; size: 10; text: parent.fright; color: root.pa.text2; x: root.fieldX + root.fieldW - 4 - implicitWidth; ty: root.listTop + 6 }
        }
        Repeater {
            model: root.cols.length
            PlatinumText {
                required property int index
                readonly property bool last: index === root.cols.length - 1 && root.cols.length > 1
                readonly property var c: root.colX[index] || [0, 0]
                face: root.face; size: 11; text: root.cols[index] ? root.cols[index][0] : ""; color: root.pa.text; ty: root.colsTop + 2
                x: last ? c[0] + c[1] - 6 - implicitWidth : c[0] + 6
            }
        }

        // ---------- rows ----------
        Item {
            id: list
            x: root.lx; y: root.rowsTop; width: root.listW; height: root.rowsH
            clip: true
            PlatinumText {
                visible: root.kind === "queue" && root.entries.length === 0
                face: root.face; text: "Nothing to show."; color: root.pa.text2; ty: 18; x: Math.round(root.listW / 2 - implicitWidth / 2)
            }
            Repeater {
                model: root.entries.slice(root.scroll, root.scroll + root.fullRows + 1)
                PlatinumRow {
                    id: pr
                    required property var modelData
                    required property int index
                    readonly property int entryIndex: root.scroll + index
                    face: root.face
                    entry: modelData
                    kind: root.kind
                    colX: root.colX.map((c) => [c[0] - root.lx, c[1]])
                    listW: root.listW
                    following: root.face.m.mode === "following"
                    selected: root.selEntry === entryIndex
                    inert: !!root.model.inert
                    keyW: root.infoKeyW
                    y: index * 16
                    onPicked: (dbl) => root.pick(pr.entry, dbl)
                    onTriangle: root.triangle(pr.entry)
                    onMenu: (mx, my) => { menuPoint.x = list.x + mx; menuPoint.y = list.y + pr.y + my; root.rowMenu(pr.entry, menuPoint); }
                }
            }
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.NoButton
                onWheel: (wheel) => root.setScroll(root.scroll + (wheel.angleDelta.y < 0 ? 3 : -3))
            }
        }
        Item { id: menuPoint; width: 1; height: 0 }

        // The foot: the hint line.
        PlatinumRects {
            visible: !!root.model.foot
            rects: { const o = [], fy = root.listBottom - 1 - root.footH; Pt.R(o, root.lx, fy, root.listW, root.footH, root.pa.face); Pt.R(o, root.lx, fy, root.listW, 1, root.pa.flat ? root.pa.ridge : root.pa.lo); return o; }
        }
        PlatinumText {
            visible: !!root.model.foot
            face: root.face; size: 10; text: root.model.foot ? root.model.foot.l : ""; color: root.pa.text2
            x: root.lx + 6; ty: root.listBottom - 1 - root.footH + 3; width: Math.min(implicitWidth, root.listW - 12); elide: Text.ElideRight
        }

        // ---------- the scroll bar ----------
        readonly property int sbY: root.listTop + 1
        readonly property int sbH: root.listBottom - root.listTop - 2
        readonly property var thumb: Pt.scrollThumb(sbH, root.maxScroll ? root.scroll / root.maxScroll : 0, root.entries.length ? Math.min(1, root.fullRows / root.entries.length) : 1)
        PlatinumRects {
            rects: Pt.scrollBar(root.pa, root.lx + root.listW, parent.sbY, parent.sbH, root.maxScroll ? root.scroll / root.maxScroll : 0,
                                root.entries.length ? Math.min(1, root.fullRows / root.entries.length) : 1, track.pressed)
        }
        Hotspot {
            x: root.lx + root.listW; y: parent.sbY; width: 15; height: 14
            tip: "Scroll up"
            onActivated: root.setScroll(root.scroll - 1)
        }
        Hotspot {
            x: root.lx + root.listW; y: parent.sbY + parent.sbH - 14; width: 15; height: 14
            tip: "Scroll down"
            onActivated: root.setScroll(root.scroll + 1)
        }
        Hotspot {
            id: track
            x: root.lx + root.listW; y: parent.sbY + 15; width: 15; height: Math.max(1, parent.sbH - 30)
            readonly property int grip: parent.thumb ? parent.thumb.size : height
            tip: root.entries.length > root.fullRows ? root.entries.length.toLocaleString(Qt.locale("en_US"), "f", 0) + " rows" : ""
            function to(my) { root.setScroll(Math.round(Drag.fraction(my, height, grip) * root.maxScroll)); }
            onBegan: to(mouseY)
            onPositionChanged: (mouse) => { if (pressed) to(mouse.y); }
        }

        // ---------- the bottom bar: pop-up buttons, Library, grow box ----------
        readonly property var bar: Pt.bottomBar(
            [fm11.advanceWidth("Add"), fm11.advanceWidth("Remove"), fm11.advanceWidth("Select"), fm11.advanceWidth("Misc"), fm11.advanceWidth("Library")],
            [fm10.advanceWidth("Add"), fm10.advanceWidth("Remove"), fm10.advanceWidth("Select"), fm10.advanceWidth("Misc"), fm10.advanceWidth("Library")])
        FontMetrics { id: fm10; font.family: root.face.chicago; font.pixelSize: 10 }
        readonly property int by: root.height - 26
        Repeater {
            model: [
                { l: "Add", menu: "add", tip: "Add to the queue" },
                { l: "Remove", menu: "rem", tip: "Remove from the queue" },
                { l: "Select", menu: "sel", tip: "Select" },
                { l: "Misc", menu: "misc", tip: "Item info, mark as played, album mode, save as playlist" },
            ]
            PlatinumButton {
                id: pop
                required property var modelData
                required property int index
                pal: root.pa
                x: parent.bar.buttons[index].x; y: parent.by; width: parent.bar.buttons[index].w; height: 18
                latched: root.face.menu === modelData.menu
                tip: modelData.tip
                onActivated: root.face.openMenu(modelData.menu, pop)
                PlatinumText { face: root.face; size: pop.parent.bar.size; text: pop.modelData.l; color: root.pa.text; x: 5 + pop.o; ty: (pop.parent.bar.size === 11 ? 3 : 4) + pop.o }
                PlatinumRects { rects: { const o = []; for (let i = 0; i < 4; i++) Pt.R(o, pop.width - 11 + i + pop.o, 8 + i + pop.o, 7 - 2 * i, 1, root.pa.glyph); return o; } }
            }
        }
        PlatinumButton {
            id: libBtn
            pal: root.pa
            x: parent.bar.buttons[4].x; y: parent.by; width: parent.bar.buttons[4].w; height: 18
            on: root.face.view === "library"
            tip: "Queue or library (L)"
            onActivated: root.face.showView("library")
            PlatinumRects { rects: { const o = []; Pt.lamp(o, root.pa, 5 + libBtn.o, 7 + libBtn.o, root.face.view === "library", false); return o; } }
            PlatinumText { face: root.face; size: libBtn.parent.bar.size; text: "Library"; color: root.pa.text; x: 14 + libBtn.o; ty: (libBtn.parent.bar.size === 11 ? 3 : 4) + libBtn.o }
        }
        PlatinumRects { rects: Pt.growBox(root.pa, 291, root.height - 9) }
        Hotspot {
            id: grow
            x: 288; y: root.height - 12; width: 12; height: 12
            tip: "Drag to change the playlist height"
            property real y0: 0
            property int n0: 0
            function absY(my) { return grow.mapToItem(root.parent, 0, my).y; }
            onBegan: { grow.y0 = absY(mouseY); grow.n0 = root.face.fit.plN; }
            onPositionChanged: (mouse) => {
                if (!pressed) return;
                const n = Math.max(0, Math.min(6, grow.n0 + Math.round((absY(mouse.y) - grow.y0) / 32)));
                if (n !== root.face.p.plN) root.face.prefs.set({ plN: n });
            }
        }
    }

    // ---------- row gestures ----------
    function pick(e, dbl) {
        if (root.model.inert) return;
        if (e.at >= 0) { root.face.setSel(e.at); if (dbl) root.face.rowEnter(e.at, false); return; }
        if (root.kind === "library") {
            // A row of a higher level: select it there, which folds what is below it.
            root.face.libFoldTo(e.level, e.idx);
            if (dbl && e.idx >= 0) root.face.rowEnter(e.idx, false);
        }
    }
    function triangle(e) {
        if (root.model.inert) return;
        if (e.tri === "open") { root.face.libFoldTo(e.level, e.idx); return; }
        if (e.at >= 0) { root.face.setSel(e.at); root.face.rowEnter(e.at, false); return; }
        root.face.libFoldTo(e.level, e.idx);
        if (e.idx >= 0) root.face.rowEnter(e.idx, false);
    }
    function rowMenu(e, anchor) {
        if (e.at >= 0) { root.face.openRowMenu(e.at, anchor); return; }
        if (root.kind === "library" && e.idx >= 0) { root.face.libFoldTo(e.level, e.idx); root.face.openRowMenu(e.idx, anchor); }
    }
}
