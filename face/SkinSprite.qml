import QtQuick
import "lib/sprites.js" as Sprites

// One sprite from assets/skin.png at 1 skin pixel per unit. Parents scale the
// whole tree by a whole number of physical pixels; smooth: false keeps the
// nearest-neighbour look at every scale.
Image {
    id: root
    property string name
    readonly property var rect: Sprites.sprites[name] || [0, 0, 1, 1]
    source: Qt.resolvedUrl("assets/skin.png")
    sourceClipRect: Qt.rect(rect[0], rect[1], rect[2], rect[3])
    width: rect[2]
    height: rect[3]
    smooth: false
    mipmap: false
    cache: true
}
