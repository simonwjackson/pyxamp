import QtQuick

// A pointer target in skin pixels: press feedback, click on release inside,
// right click, and its tooltip. Disabled hotspots still show their tooltip,
// which says why they do nothing.
MouseArea {
    id: root
    property string tip: ""
    property bool active: true          // false: dimmed, no action
    readonly property bool down: pressed && containsMouse && active && pressedButtons & Qt.LeftButton
    signal began()                      // the gesture starts: capture its target here
    signal activated()
    signal secondary(real x, real y)
    hoverEnabled: true
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    onPositionChanged: (mouse) => { if (!pressed) Tips.hover(root, root.tip, mouse.x, mouse.y); }
    onExited: Tips.clear()
    onTipChanged: if (containsMouse && Tips.anchor === root) Tips.hover(root, root.tip, mouseX, mouseY)
    onPressed: (mouse) => {
        Tips.clear();
        if (mouse.button === Qt.RightButton) { root.secondary(mouse.x, mouse.y); return; }
        if (root.active) root.began();
    }
    onReleased: (mouse) => {
        if (mouse.button === Qt.LeftButton && root.active && containsMouse) root.activated();
    }
}
