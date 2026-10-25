import QtQuick
import Quickshell
import Quickshell.Io
import "lib/prefs.js" as PrefsModel

// Local view preferences at $XDG_STATE_HOME/pyxamp/face.json (or
// ~/.local/state/pyxamp/face.json). Non-sensitive only: positions per monitor,
// folds, double size, layer, vis mode, time mode, EQ sliders, reduced motion.
// PYXAMP_FACE_STATE names another file (the face's checks use a scratch one).
Item {
    id: root
    visible: false

    readonly property string path: {
        const override = Quickshell.env("PYXAMP_FACE_STATE");
        if (override) return override;
        const state = Quickshell.env("XDG_STATE_HOME") || (Quickshell.env("HOME") + "/.local/state");
        return state + "/pyxamp/face.json";
    }
    property var value: PrefsModel.defaults()

    function set(changes) {
        root.value = PrefsModel.withChanges(root.value, changes);
        save.restart();
    }

    FileView {
        id: file
        path: root.path
        blockLoading: true
        printErrors: false
        atomicWrites: true
        onLoaded: {
            let raw = null;
            try { raw = JSON.parse(file.text()); } catch (e) { raw = null; }
            root.value = PrefsModel.sanitize(raw);
        }
        onSaveFailed: (error) => console.warn("pyxamp: cannot save " + root.path + ": " + FileViewError.toString(error))
    }

    Timer {
        id: save
        interval: 300
        onTriggered: file.setText(JSON.stringify(root.value, null, 1) + "\n")
    }

    Component.onCompleted: {
        // FileView writes the file but not its directory.
        Quickshell.execDetached(["mkdir", "-p", root.path.replace(/\/[^\/]*$/, "")]);
    }
}
