.pragma library
// Every slider's pointer target on both faces, in section coordinates (sp):
// { x, y, w, h, grip, axis }. grip is the part of the target's length the
// thumb and its margins take (lib/drag.js). The sections read their targets
// from here, and the unit tests check each one maps a pointer on the drawn
// thumb back to the thumb's value, so no face can drift back to hard-coded
// drag origins.

var winamp = {
    // MainSection.qml: the knob is 14 sp wide, the seek thumb 29 sp.
    volume: { x: 107, y: 56, w: 68, h: 13, grip: 14, axis: "x" },
    balance: { x: 177, y: 56, w: 38, h: 13, grip: 14, axis: "x" },
    seek: { x: 16, y: 72, w: 248, h: 10, grip: 29, axis: "x" },
    // EqSection.qml: one per slider at x = 21 (preamp) or 78 + 18 * band; the knob is 11 sp tall.
    eq: { x: 21, y: 38, w: 14, h: 63, grip: 11, axis: "y", pitch: 18, bandX: 78 },
};

// The Platinum face (the approved mock's hslider(), vslider()): the target is
// 2 sp wider than the groove on each side, the thumb 11 sp wide, so grip is 15.
function hslider(x, y, w) { return { x: x - 2, y: y - 1, w: w + 4, h: 16, grip: 15, axis: "x", groove: { x: x, y: y, w: w } }; }
var platinum = {
    seek: hslider(8, 84, 284),
    volume: hslider(226, 98, 66),
    balance: hslider(226, 112, 66),
    speed: hslider(236, 112, 56),
    // PlatinumEq.qml: preamp at x = 9, bands at 60 + 23 * band; the groove is 62 sp tall,
    // the thumb's centre travels 60 sp of it, so the 66-sp target has a grip of 6.
    eq: { x: 8, y: 41, w: 20, h: 66, grip: 6, axis: "y", pitch: 23, bandX: 59, grooveTop: 43, grooveH: 62 },
};

// The EQ target of slider i (0 = preamp, 1..10 = bands) on a face.
function eqTarget(face, i) {
    var e = face === "platinum" ? platinum.eq : winamp.eq;
    var out = {};
    for (var k in e) out[k] = e[k];
    if (i > 0) out.x = e.bandX + (i - 1) * e.pitch;
    return out;
}
