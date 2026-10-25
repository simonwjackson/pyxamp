import QtQuick

// The face's monotonic clock, in milliseconds. It is QtQuick's FrameAnimation
// elapsedTime: Qt sums each frame's QElapsedTimer delta (CLOCK_MONOTONIC on
// Linux), so a wall-clock change, NTP step or time-zone change cannot move it.
// Checked in qtdeclarative 6.11.2 src/quick/util/qquickframeanimation.cpp.
//
// It runs only while something needs time (playing, a scrolling marquee, the
// visualizer, a confirmation on the display): a running animation makes Qt
// render every frame. Stopped, it does not count. Every use reads differences
// taken while it runs: the daemon sends a new snapshot (a new base) on every
// transport change, so position is never extrapolated across a stop.
Item {
    id: root
    visible: false
    property bool animating: false
    // Published at most every 33 ms, so bindings do not re-evaluate every frame.
    property real ms: 0

    function now() { return frames.elapsedTime * 1000; }

    FrameAnimation {
        id: frames
        running: root.animating
        onTriggered: {
            const t = frames.elapsedTime * 1000;
            if (t - root.ms >= 33) root.ms = t;
        }
        onRunningChanged: if (!running) root.ms = frames.elapsedTime * 1000
    }
}
