import QtQuick

// Dragging a title bar moves the whole column on its monitor; the place is
// remembered for that monitor on release. Double click toggles windowshade.
MouseArea {
    id: root
    required property var face
    property real grabX: 0
    property real grabY: 0
    property var start: null
    acceptedButtons: Qt.LeftButton
    onPressed: (mouse) => { Tips.clear(); root.grabX = mouse.x; root.grabY = mouse.y; root.start = { x: face.place.x, y: face.place.y }; }
    onPositionChanged: (mouse) => {
        if (!pressed || !root.start) return;
        // The window moves under the pointer, so the pointer's offset from where it
        // grabbed is the move still to make (skin px -> logical px).
        const s = face.fit.contentScale;
        const cur = face.dragPos || root.start;
        face.dragPos = { x: Math.round(cur.x + (mouse.x - root.grabX) * s), y: Math.round(cur.y + (mouse.y - root.grabY) * s) };
    }
    onReleased: {
        if (face.dragPos) face.rememberPosition(face.dragPos);
        face.dragPos = null;
        root.start = null;
    }
    onDoubleClicked: face.prefs.set({ shade: !face.p.shade })
}
