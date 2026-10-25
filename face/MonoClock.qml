import QtQuick

// The face's monotonic clock, in milliseconds. It is QtQuick's FrameAnimation
// elapsedTime: Qt sums each frame's QElapsedTimer delta (CLOCK_MONOTONIC on
// Linux), so a wall-clock change, NTP step or time-zone change cannot move it.
// Checked in qtdeclarative 6.11.2 src/quick/util/qquickframeanimation.cpp.
//
// It is paused, never stopped, while nothing needs time: a running animation
// makes Qt render every frame. Pausing keeps elapsedTime; stopping does not
// (setRunning() sets firstTick, and the next start sets elapsedTime back to 0,
// which once made the clock jump back and froze every confirmation). Time
// spent paused is not counted. Every use reads differences taken while it
// runs: the daemon sends a new snapshot (a new base) on every transport
// change, so position is never extrapolated across a pause.
Item {
    id: root
    visible: false
    property bool animating: false
    // Published at most every 33 ms, so bindings do not re-evaluate every frame.
    property real ms: 0

    function now() { return frames.elapsedTime * 1000; }

    FrameAnimation {
        id: frames
        running: true
        paused: !root.animating
        onTriggered: {
            const t = frames.elapsedTime * 1000;
            if (t - root.ms >= 33) root.ms = t;
        }
        onPausedChanged: if (paused) root.ms = frames.elapsedTime * 1000
    }
}
