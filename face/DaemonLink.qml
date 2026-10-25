import QtQuick
import Quickshell
import Quickshell.Io
import "lib/protocol.js" as Protocol

// The face's only data edge: one client of the daemon's control socket.
// It says hello, keeps the latest snapshot and visualizer frame, sends
// commands, and reconnects after a loss. On loss it drops the snapshot, so no
// control acts on stale state, and it never resends a command.
//
// The production path is $XDG_RUNTIME_DIR/pyxamp/control.sock. PYXAMP_SOCKET
// names another socket (the face's checks point it at their replay server);
// the protocol and everything above this file stay the same.
Item {
    id: root
    visible: false

    required property MonoClock clock
    property string socketPath: {
        const override = Quickshell.env("PYXAMP_SOCKET");
        if (override) return override;
        const runtime = Quickshell.env("XDG_RUNTIME_DIR");
        return runtime ? runtime + "/pyxamp/control.sock" : "";
    }

    // { _tag: "connecting" } | { _tag: "connected", instanceId } | { _tag: "disconnected", reason }
    property var link: ({ _tag: "connecting" })
    property var player: null
    property real observedAt: 0
    property int revision: -1
    property var frame: null
    property string instanceId: ""

    // A command's outcome: (id, { _tag: "accepted" } | { _tag: "rejected", code, message }).
    signal outcome(string id, var result)

    property int nextId: 1
    property int backoff: 250

    function command(target, action) {
        if (root.link._tag !== "connected" || !target) return "";
        const id = "face-" + Quickshell.processId + "-" + (root.nextId++);
        conn.item.write(Protocol.command(id, target, action) + "\n");
        conn.item.flush();
        return id;
    }

    function lose(reason) {
        root.link = { _tag: "disconnected", reason: reason };
        root.player = null;
        root.frame = null;
        root.revision = -1;
        // Close the old connection at once (not from inside its own handler).
        Qt.callLater(() => { if (root.link._tag === "disconnected") conn.active = false; });
        root.scheduleRetry();
    }

    function handle(line) {
        if (root.link._tag === "disconnected") return; // a lost connection's last lines
        const d = Protocol.decodeServer(line);
        if (!d.ok) { root.lose("the daemon sent a message this face cannot read (" + d.reason + ")"); return; }
        const m = d.msg;
        switch (m._tag) {
        case "ready":
            root.instanceId = m.instanceId;
            root.revision = -1;
            root.backoff = 250;
            root.link = { _tag: "connected", instanceId: m.instanceId };
            break;
        case "state":
            if (root.link._tag !== "connected") return;
            root.revision = m.revision;
            root.observedAt = root.clock.now();
            root.player = m.player;
            break;
        case "visualizer":
            if (root.link._tag !== "connected") return;
            if (root.frame && m.sequence < root.frame.sequence && m.target && root.frame.target && Protocol.sameTarget(m.target, root.frame.target)) return;
            root.frame = m;
            break;
        case "result":
            root.outcome(m.id, m.outcome);
            break;
        case "error":
            console.warn("pyxamp: daemon error " + m.code + ": " + m.message);
            if (root.link._tag !== "connected") root.lose("the daemon refused this face: " + m.message);
            break;
        }
    }

    // One Socket per attempt: a Socket that failed once does not retry reliably.
    Loader {
        id: conn
        active: false
        sourceComponent: Socket {
            path: root.socketPath
            connected: true
            parser: SplitParser { onRead: (data) => root.handle(data) }
            onConnectedChanged: {
                if (connected) {
                    root.link = { _tag: "connecting" };
                    write(Protocol.hello() + "\n");
                    flush();
                } else if (root.link._tag !== "disconnected") {
                    root.lose("the connection closed");
                }
            }
            onError: (error) => {
                if (root.link._tag !== "disconnected") root.lose(root.socketPath ? "no daemon at " + root.socketPath : "XDG_RUNTIME_DIR is not set");
                else root.scheduleRetry();
            }
        }
    }

    function scheduleRetry() {
        retry.interval = root.backoff;
        root.backoff = Math.min(5000, root.backoff * 2);
        retry.restart();
    }

    Timer {
        id: retry
        onTriggered: { conn.active = false; if (root.socketPath) conn.active = true; }
    }

    Component.onCompleted: if (root.socketPath) conn.active = true; else root.lose("XDG_RUNTIME_DIR is not set")
}
