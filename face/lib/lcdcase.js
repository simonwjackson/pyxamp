.pragma library
// Mixed case for the Platinum face's LCD and headers. Pure.
//
// The shared model writes status messages in upper case, because the Winamp
// face's 5x6 font has no lower case. ChicagoFLF has, so the Platinum face
// turns each all-caps part into sentence case ("SPEED: 1.5X" -> "Speed: 1.5x",
// "BACK TO PYXAMP: STAYS PAUSED" -> "Back to Pyxamp: stays paused"). A part
// that already has lower case (a title, a reason from the daemon) is kept, so
// only the model's own words change. Proper nouns come from a small fixed list
// and from the names the state carries (the followed app, the output devices).
// Ported from the approved mock's macCase() (/tmp/macamp-mock/mac.js).

var PROPER = {
    PYXAMP: "Pyxamp", PYXIS: "Pyxis", JELLYFIN: "Jellyfin", OSSICLE: "Ossicle", FIREFOX: "Firefox", CHROMIUM: "Chromium",
    YOUTUBE: "YouTube", EQ: "EQ", HTTP: "HTTP", MPV: "mpv", DB: "dB", VOICE: "Voice", MPRIS: "MPRIS", PIPEWIRE: "PipeWire",
    SONOS: "Sonos", X: "X", J: "J", L: "L", R: "R", S: "S", I: "I",
};
var SEP = "  ***  ";

function hasLetters(s) { return /[A-Za-z]/.test(s); }
function allCaps(s) { var l = s.replace(/[^A-Za-z]/g, ""); return l !== "" && l === l.toUpperCase(); }

function restoreNames(s, names) {
    for (var i = 0; i < names.length; i++) {
        var n = names[i];
        if (!n) continue;
        s = s.split(n.toUpperCase()).join(n).split(n.toLowerCase()).join(n);
    }
    return s;
}

// text: one status line. names: proper names in their own case (["Living Room", "Firefox"]).
function sentence(text, names) {
    names = names || [];
    var s = String(text || "");
    return s.split(SEP).map(function (seg) {
        if (!hasLetters(seg)) return seg;
        if (!allCaps(seg)) {
            // "NEXT: Chromium does not offer this": the model's label before the reason.
            var m = /^([^a-z]*[A-Z][^a-z]*?): (.*)$/.exec(seg);
            if (m && allCaps(m[1]) && !allCaps(m[2])) return sentence(m[1], names) + ": " + restoreNames(m[2], names);
            return restoreNames(seg, names);
        }
        var out = seg.toLowerCase().replace(/[a-z][a-z']*/g, function (w) { return PROPER[w.toUpperCase()] || w; });
        out = out.replace(/(^|[.!?]\s+)([a-z])/g, function (m, a, b) { return a + b.toUpperCase(); });
        out = out.replace(/(\d)khz\b/g, "$1 kHz").replace(/(\d)hz\b/g, "$1 Hz").replace(/(\d)X\b/g, "$1x").replace(/(\d) S\b/g, "$1 s");
        return restoreNames(out, names);
    }).join(SEP);
}

// A header title from the model ("PYXIS QUEUE" -> "Pyxis queue").
function title(text) { return sentence(text, []); }
