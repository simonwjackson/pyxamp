import QtQuick
import Quickshell
import Quickshell.Io
import "lib/desktop-theme.js" as DT

// The desktop theme, read the way the desktop reads it: which face Pyxamp
// wears (the theme's pyxampFace in mountainous's theme registry) and its
// appearance (light, dark grey, extra dark). The rule is lib/desktop-theme.js;
// this file only reads and watches the four files. desktop-theme replaces them
// with a rename, so each file is watched, and also read again every 2 s, so a
// change always arrives even when a watch was lost with the old file.
Scope {
    id: root

    readonly property var env: ({
        PYXAMP_DESKTOP_THEMES: Quickshell.env("PYXAMP_DESKTOP_THEMES") || "",
        XDG_CONFIG_HOME: Quickshell.env("XDG_CONFIG_HOME") || "",
        XDG_STATE_HOME: Quickshell.env("XDG_STATE_HOME") || "",
        HOME: Quickshell.env("HOME") || "",
    })
    readonly property var paths: DT.paths(root.env)

    // A file's text, or null while it is missing or unreadable.
    property var nameText: null
    property var registryText: null
    property var modeText: null
    property var extraText: null
    function keep(which, value) { if (root[which] !== value) root[which] = value; }

    readonly property string face: DT.face(root.nameText, root.registryText)
    readonly property string themeName: DT.themeName(root.nameText, DT.parseRegistry(root.registryText))
    // Theme.qml keeps the mode it shows when the file holds something else.
    property var look: DT.appearance(null, null, "")
    function relook() {
        const next = DT.appearance(root.modeText, root.extraText, root.look.mode);
        if (next.appearance !== root.look.appearance || next.mode !== root.look.mode || next.extraDark !== root.look.extraDark) root.look = next;
    }
    onModeTextChanged: relook()
    onExtraTextChanged: relook()
    readonly property string appearance: root.look.appearance

    component Watched: FileView {
        required property string slot
        watchChanges: true
        printErrors: false
        blockLoading: true
        onFileChanged: reload()
        onLoaded: root.keep(slot, text())
        onLoadFailed: root.keep(slot, null)
    }
    Watched { id: nameFile; slot: "nameText"; path: root.paths.name }
    Watched { id: registryFile; slot: "registryText"; path: root.paths.registry }
    Watched { id: modeFile; slot: "modeText"; path: root.paths.mode }
    Watched { id: extraFile; slot: "extraText"; path: root.paths.extra }

    Timer {
        interval: 2000
        repeat: true
        running: true
        onTriggered: { nameFile.reload(); registryFile.reload(); modeFile.reload(); extraFile.reload(); }
    }
}
