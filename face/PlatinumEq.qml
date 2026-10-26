import QtQuick
import "lib/platinum.js" as Pt
import "lib/lcdtext.js" as Lcd
import "lib/lcdcase.js" as Case
import "lib/sliders.js" as Sliders
import "lib/drag.js" as Drag
import "lib/eq.js" as Eq

// The Platinum face's EQ section: a 17-sp disclosure header that stays when
// the section is folded, then (122 sp in all) On and Auto, the curve on a small
// LCD, the Presets pop-up and 11 vertical Platinum sliders. It changes
// Pyxamp's own audio only, exactly as the Winamp face's EQ section does.
Item {
    id: root
    required property var face
    property bool active: true
    readonly property bool open: face.p.eqOpen
    readonly property var pa: face.pal
    readonly property var lc: face.lcdSet
    readonly property var eq: face.eqShown
    readonly property string note: face.m.eqNote || ""
    readonly property bool off: !eq.on || note !== ""
    width: 300
    height: open ? 122 : 17

    Rectangle { x: 1; width: 298; height: root.height; color: root.pa.face }
    PlatinumRects { rects: Pt.header(root.pa, root.open) }
    PlatinumText { id: label; face: root.face; text: "Equalizer"; x: 24; ty: 3; color: root.pa.text }
    Hotspot {
        x: 4; y: 1; width: Math.min(140, Math.ceil(label.implicitWidth) + 30); height: 16
        tip: root.open ? "Fold the equalizer (Alt+G)" : "Unfold the equalizer (Alt+G)"
        onActivated: root.face.prefs.set({ eqOpen: !root.open })
    }

    Item {
        visible: root.open
        // On and Auto, with lamps.
        PlatinumButton {
            id: onBtn
            pal: root.pa
            x: 8; y: 19; width: 36; height: 17
            on: root.eq.on
            tip: "Equalizer on or off"
            onActivated: { root.face.setEq({ on: !root.eq.on }); root.face.say("EQ: " + (root.eq.on ? "ON" : "OFF"), 1000); }
            PlatinumRects { rects: { const o = []; Pt.lamp(o, root.pa, 5 + onBtn.o, 6 + onBtn.o, root.eq.on, false); return o; } }
            PlatinumText { face: root.face; size: 11; text: "On"; x: 13 + onBtn.o; ty: 2 + onBtn.o; color: root.pa.text }
        }
        PlatinumButton {
            id: autoBtn
            pal: root.pa
            x: 47; y: 19; width: 46; height: 17
            on: root.face.p.eq.auto
            tip: "Auto: a preset per kind of audio (music or spoken word)"
            onActivated: { root.face.prefs.set({ eq: Object.assign({}, root.face.p.eq, { auto: !root.face.p.eq.auto }) }); root.face.say("EQ AUTO: " + (root.face.p.eq.auto ? "OFF" : "ON"), 1000); }
            PlatinumRects { rects: { const o = []; Pt.lamp(o, root.pa, 5 + autoBtn.o, 6 + autoBtn.o, root.face.p.eq.auto, false); return o; } }
            PlatinumText { face: root.face; size: 11; text: "Auto"; x: 13 + autoBtn.o; ty: 2 + autoBtn.o; color: root.pa.text }
        }

        // The curve, on a small LCD (126 x 17 at 98, 19).
        readonly property var curve: Pt.eqCurve(root.eq.bands, 120, 4, 13)
        PlatinumRects {
            rects: {
                const o = [], L = root.lc;
                Pt.sunken(o, root.pa, 97, 19, 128, 17, "");
                Pt.R(o, 98, 20, 126, 15, root.pa.flat ? "#000000" : "#000000");
                Pt.R(o, 99, 21, 124, 13, L.bg);
                if (root.note === "") {
                    Pt.R(o, 100, 27 - Math.round(root.eq.pre / 12 * 5), 122, 1, L.ghost);
                    const cv = parent.curve;
                    for (let x = 0; x < cv.length; x++) Pt.R(o, 101 + x, 27 - cv[x], 1, 2, root.off ? L.dim : L.ink);
                }
                return o;
            }
        }
        Item {
            x: 99; y: 21; width: 124; height: 13
            clip: true
            readonly property string s: Case.sentence(root.note, [root.face.m.identity])
            PlatinumLcdText { visible: root.note !== ""; x: Math.max(1, Math.floor((126 - width) / 2) - 1); text: parent.s; ink: root.lc.ink }
        }
        Hotspot {
            x: 98; y: 19; width: 126; height: 17
            tip: root.note ? "The equalizer changes Pyxamp's own audio only, not " + root.face.m.identity : root.eq.auto ? "Auto: Voice for spoken word" : "The equalizer changes Pyxamp's own audio"
        }

        // Presets pop-up.
        PlatinumButton {
            id: presets
            pal: root.pa
            x: 228; y: 19; width: 64; height: 17
            latched: root.face.menu === "presets"
            tip: "Presets"
            onActivated: root.face.openMenu("presets", presets)
            PlatinumText { face: root.face; size: 11; text: "Presets"; x: 5 + presets.o; ty: 2 + presets.o; color: root.pa.text }
            PlatinumRects { rects: { const o = []; Pt.R(o, 49, 3, 1, 11, root.pa.flat ? root.pa.btnEdge : root.pa.lo); Pt.tri(o, 52 + presets.o, 5 + presets.o, "down", root.pa.glyph); return o; } }
        }

        // Preamp and ten bands: vertical sliders with ticks and a fill from 0 dB.
        PlatinumText { face: root.face; size: 10; text: "+12"; color: root.pa.text2; x: 54 - implicitWidth; ty: 42 }
        PlatinumText { face: root.face; size: 10; text: "0"; color: root.pa.text2; x: 54 - implicitWidth; ty: 69 }
        PlatinumText { face: root.face; size: 10; text: "\u221212"; color: root.pa.text2; x: 54 - implicitWidth; ty: 97 }
        Repeater {
            model: 11
            Item {
                id: slider
                required property int index
                readonly property bool pre: index === 0
                readonly property int band: index - 1
                readonly property int db: pre ? root.eq.pre : root.eq.bands[band]
                readonly property var r: Sliders.eqTarget("platinum", index)
                readonly property int sx: r.x + 1                   // the slider's drawing origin
                PlatinumRects {
                    rects: { const o = []; Pt.vslider(o, root.pa, slider.sx, slider.r.grooveTop, slider.r.grooveH, slider.db, { off: root.off, pressed: area.pressed }); return o; }
                }
                PlatinumText {
                    face: root.face; size: 10
                    text: slider.pre ? "Pre" : Eq.BAND_LABELS[slider.band]
                    color: root.off ? root.pa.off : root.pa.text
                    x: slider.sx + 8 - Math.round(implicitWidth / 2); ty: slider.r.grooveTop + slider.r.grooveH + 3
                }
                Hotspot {
                    id: area
                    x: slider.r.x; y: slider.r.y; width: slider.r.w; height: slider.r.h
                    tip: slider.pre ? "Preamp" : Eq.BAND_LABELS[slider.band].replace("K", " k") + "Hz"
                    function apply(my) {
                        const v = Drag.dbAt(my, height, slider.r.grip, -12, 12);
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
}
