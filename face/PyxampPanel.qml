import QtQuick
import Quickshell
import Quickshell.Wayland

// The one Pyxamp surface: a layer-shell window holding the main, EQ and
// playlist sections in Winamp's docked column. Drawn at 1 skin pixel per unit
// and scaled to k physical px per skin pixel; the compositor's scale is
// divided out here, once (lib/fit.js contentScale). Folds, windowshade and
// SUPER+Z change it at once, with no animation.
PanelWindow {
    id: win
    required property var face
    readonly property real dpr: win.devicePixelRatio > 0 ? win.devicePixelRatio : face.monitorScale

    screen: face.screen
    visible: face.p.shown && face.screen !== null
    anchors { top: true; left: true }
    margins { left: face.place.x; top: face.place.y }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.layer: face.p.onTop ? WlrLayer.Top : WlrLayer.Bottom
    WlrLayershell.namespace: "pyxamp"
    focusable: true                     // layer-shell keyboard focus on demand: a click gives it the keys
    color: "transparent"
    implicitWidth: face.fit.logicalW
    implicitHeight: face.fit.logicalH

    Item {
        id: surface
        width: 275
        height: face.fit.spH
        transformOrigin: Item.TopLeft
        scale: face.fit.k / win.dpr
        focus: true
        Keys.onPressed: (event) => { if (win.face.key(event)) event.accepted = true; }

        Column {
            MainSection { face: win.face; active: surface.activeFocus; visible: !win.face.p.shade }
            ShadeSection { face: win.face; active: surface.activeFocus; visible: win.face.p.shade }
            EqSection { face: win.face; active: surface.activeFocus; visible: win.face.p.eqOpen }
            PlaylistSection { face: win.face; active: surface.activeFocus; visible: win.face.p.plOpen; steps: win.face.fit.plN }
        }
    }

    PopupMenu {
        face: win.face
        kind: win.face.menu
        anchorItem: win.face.menuAnchor
        scaleFactor: surface.scale
        onDismissed: win.face.menu = ""
    }

    Tooltip { host: win; scaleFactor: surface.scale }
}
