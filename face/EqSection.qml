import QtQuick
import "lib/eq.js" as Eq
import "lib/tokens.js" as Tokens

// The EQ section (275 x 116 sp). It changes Pyxamp's own audio only; while
// following another player the graph says NO EQ ON <APP> and the sliders dim,
// but stay editable (decision 18).
Item {
    id: root
    required property var face
    property bool active: true
    readonly property var eq: face.p.eq
    readonly property string note: face.m.eqNote || ""
    readonly property bool off: !eq.on || note !== ""
    width: 275
    height: 116

    SkinSprite { name: "eq-bg" }
    SkinSprite { x: 1; y: 1; name: root.active ? "eq-title-on" : "eq-title-off" }
    TitleDrag { x: 6; y: 1; width: 236; height: 14; face: root.face }
    TitleButton { x: 253; y: 4; kind: "shade"; tip: "Fold the equalizer (Alt+G)"; onActivated: root.face.prefs.set({ eqOpen: false }) }
    TitleButton { x: 263; y: 4; kind: "close"; tip: "Fold the equalizer (Alt+G)"; onActivated: root.face.prefs.set({ eqOpen: false }) }

    SkinButton { x: 14; y: 18; face: "btn-on-" + (root.eq.on ? 1 : 0); tip: "Equalizer on or off"; onActivated: { root.face.setEq({ on: !root.eq.on }); root.face.say("EQ: " + (root.eq.on ? "ON" : "OFF"), 1000); } }
    SkinButton { x: 40; y: 18; face: "btn-auto-" + (root.eq.auto ? 1 : 0); tip: "Auto: a preset per kind of audio"; onActivated: root.face.setEq({ auto: !root.eq.auto }) }
    SkinButton { x: 217; y: 18; face: "btn-presets"; latched: root.face.menu === "presets"; tip: "Presets"; onActivated: root.face.openMenu("presets", this) }

    // Curve graph (113 x 19 sp at 86, 17).
    Item {
        x: 86; y: 17; width: 113; height: 19
        Rectangle { y: Eq.preampRow(root.eq.pre); width: 113; height: 1; color: root.off ? Tokens.preampOff : Tokens.preampOn }
        Repeater {
            model: Eq.curve(root.eq.bands, root.off)
            Rectangle { required property var modelData; required property int index; x: index; y: modelData.y; width: 1; height: 1; color: modelData.color }
        }
        Rectangle {
            visible: root.note !== ""
            x: Math.floor((113 - root.note.length * 5) / 2) - 2; y: 6; width: root.note.length * 5 + 3; height: 8
            color: Tokens.graphBg
            Text5 { x: 2; y: 1; text: root.note; colour: "gold" }
        }
        Hotspot { anchors.fill: parent; tip: root.note ? "The equalizer changes Pyxamp's own audio only, not " + root.face.m.identity : "The equalizer changes Pyxamp's own audio only" }
    }

    // Preamp and ten bands.
    Repeater {
        model: 11
        Item {
            id: slider
            required property int index
            readonly property bool pre: index === 0
            readonly property int band: index - 1
            readonly property int db: pre ? root.eq.pre : root.eq.bands[band]
            readonly property int sx: pre ? 21 : 78 + band * 18
            SkinSprite { x: slider.sx + 5; y: 39; name: "eqt-" + slider.db + (root.off ? "-d" : "") }
            SkinSprite { x: slider.sx + 1; y: Eq.knobY(slider.db); name: area.pressed ? "eqk-p" : "eqk" }
            Hotspot {
                id: area
                x: slider.sx; y: 38; width: 14; height: 63
                tip: slider.pre ? "Preamp" : Eq.BAND_LABELS[slider.band].replace("K", " k") + "Hz"
                function apply(my) {
                    const v = Eq.dbAt(my);
                    if (slider.pre) root.face.setEq({ pre: v });
                    else { const b = root.eq.bands.slice(); b[slider.band] = v; root.face.setEq({ bands: b }); }
                    root.face.say("EQ: " + (slider.pre ? "PREAMP" : Eq.BAND_LABELS[slider.band] + "HZ") + ": " + (v >= 0 ? "+" : "") + v.toFixed(1) + " DB", 1000);
                }
                onBegan: apply(mouseY)
                onPositionChanged: (mouse) => { if (pressed) apply(mouse.y); }
            }
        }
    }
}
