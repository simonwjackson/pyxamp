import QtQuick

// A 9x9 title-bar button. Its normal face is part of the title-bar sprite;
// only the pressed face is drawn here (also while latched: album mode on).
Hotspot {
    id: root
    property string kind: "close"       // min | shade | unshade | close | album
    property bool latched: false
    width: 9
    height: 9
    SkinSprite { name: "tb-" + root.kind + "-p"; visible: root.down || root.latched }
}
