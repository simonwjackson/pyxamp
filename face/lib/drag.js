.pragma library
// Slider drags, mapped from each control's own rectangle. Pure.
//
// A slider's pointer target is `length` sp long along its axis. The thumb (with
// any margin the target adds around the groove) takes `grip` sp of it, so the
// thumb's centre travels `length - grip` sp. A pointer at `pos` (sp, measured
// from the target's own origin, as a MouseArea reports it) maps to the
// fraction whose drawn thumb is centred under it. Nothing here knows where a
// control sits in its section, so both faces use the same mapping (ADR 0006:
// the old hard-coded Winamp origins, "(x + mx - 114) / 54", broke on any
// other layout).

function clamp01(v) { return Math.max(0, Math.min(1, v)); }

// pos: pointer offset along the axis inside the target; -> 0..1.
function fraction(pos, length, grip) {
    var travel = length - grip;
    if (travel <= 0) return 0;
    return clamp01((pos - Math.floor(grip / 2)) / travel);
}

// Where the thumb is drawn (its leading edge, sp from the target's origin) for a fraction.
function thumbOffset(f, length, grip) {
    return Math.round(clamp01(f) * Math.max(0, length - grip));
}

// The pointer offset whose fraction is f (the thumb's centre): for checks.
function pointerFor(f, length, grip) {
    return Math.floor(grip / 2) + clamp01(f) * Math.max(0, length - grip);
}

// A vertical slider whose top is the maximum (the EQ): -> dB in whole steps.
function dbAt(pos, length, grip, min, max) {
    var f = fraction(pos, length, grip);
    return Math.max(min, Math.min(max, Math.round(max - f * (max - min))));
}
function dbFraction(db, min, max) { return clamp01((max - db) / (max - min)); }
