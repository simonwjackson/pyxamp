import QtQuick

// A monotonic millisecond clock for the face. QML exposes only the wall clock,
// so this one advances by wall-clock steps and refuses steps that go back or
// jump forward by more than 2 s (a clock change or a suspend), which it counts
// as one tick. The tick runs fast only while something moves.
Item {
    id: root
    visible: false
    property bool animating: false
    property real ms: 0
    property real lastWall: Date.now()

    function now() {
        const wall = Date.now();
        let d = wall - root.lastWall;
        if (d < 0 || d > 2000) d = ticker.interval;
        root.lastWall = wall;
        root.ms += d;
        return root.ms;
    }

    Timer {
        id: ticker
        interval: root.animating ? 33 : 250
        running: true
        repeat: true
        onTriggered: root.now()
    }
}
