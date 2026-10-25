.pragma library
// Visualizer display physics for the daemon's real samples. Pure.
// 75 spectrum columns (0..255) become Winamp's 19 thick bands of 3 sp, 16 sp
// tall. Bars fall 1.25 sp per 33 ms tick; peaks hold 6 ticks, then fall with
// speed 0.12 sp/tick growing x1.12 per tick (the mock's motion table).
// Nothing here makes up samples: with no frame the bars only fall.

var BANDS = 19, HEIGHT = 16, TICK_MS = 33;

function empty() {
    var z = [];
    for (var i = 0; i < BANDS; i++) z.push(0);
    return { bars: z.slice(), peaks: z.slice(), vel: z.slice(), hold: z.slice() };
}

// Band i takes the loudest of columns 4i..4i+3 (the last band has three).
function bands(columns) {
    var out = [];
    for (var i = 0; i < BANDS; i++) {
        var m = 0;
        for (var c = i * 4; c < Math.min(75, i * 4 + 4); c++) m = Math.max(m, columns[c]);
        out.push(m / 255 * HEIGHT);
    }
    return out;
}

// One 33 ms tick. target: 19 heights, or null for silence.
function step(s, target) {
    var n = { bars: [], peaks: [], vel: [], hold: [] };
    for (var i = 0; i < BANDS; i++) {
        var bar = Math.max(target ? target[i] : 0, s.bars[i] - 1.25);
        var peak = s.peaks[i], vel = s.vel[i], hold = s.hold[i];
        if (bar >= peak) { peak = bar; vel = 0.12; hold = 6; }
        else if (hold > 0) hold--;
        else { peak = Math.max(0, peak - vel); vel *= 1.12; }
        n.bars.push(bar); n.peaks.push(peak); n.vel.push(vel); n.hold.push(hold);
    }
    return n;
}

// Oscilloscope: waveform (128 = centre) -> one row 0..15 per column.
function scopeRows(waveform) {
    var out = [];
    for (var i = 0; i < waveform.length; i++) out.push(Math.max(0, Math.min(HEIGHT - 1, Math.round(HEIGHT / 2 + (waveform[i] - 128) / 16))));
    return out;
}

// Vertical segments joining each column's row to the one before, as the mock draws them.
function scopeSegments(rows) {
    var out = [];
    for (var i = 0; i < rows.length; i++) {
        var prev = i === 0 ? rows[i] : rows[i - 1];
        out.push({ x: i, top: Math.min(prev, rows[i]), height: Math.abs(prev - rows[i]) + 1 });
    }
    return out;
}

function isQuiet(s) {
    for (var i = 0; i < BANDS; i++) if (s.bars[i] > 0 || s.peaks[i] > 0) return false;
    return true;
}
