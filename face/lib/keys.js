.pragma library
// Keyboard precedence for the Pyxamp window. Pure: a key and the view context
// in, one action (or null: not ours) out. Order, first match wins:
//   1. the jump box owns every key without Ctrl/Alt/Meta while it is open:
//      text, Backspace, Enter, Esc, and the arrows (its caret stays at the end,
//      and it has no results to move through), so nothing reaches seek,
//      volume or transport;
//   2. Ctrl/Alt chords (Ctrl+D, Ctrl+T, Alt+W, Alt+G, Alt+E, Alt+3);
//   3. any other chord is not ours;
//   4. plain keys: Winamp's Z X C V B, J, L, R, S, arrows, Esc, Backspace.
// Open popup menus own the keyboard (their own window has focus), so they
// never reach this table.

// Qt key codes and modifier bits used here (Qt::Key, Qt::KeyboardModifier).
var QT = {
    Escape: 0x01000000, Tab: 0x01000001, Backspace: 0x01000003, Return: 0x01000004, Enter: 0x01000005, Insert: 0x01000006,
    Left: 0x01000012, Up: 0x01000013, Right: 0x01000014, Down: 0x01000015,
    Shift: 0x02000000, Control: 0x04000000, Alt: 0x08000000, Meta: 0x10000000,
};
var NAMED = {};
NAMED[QT.Escape] = "Escape"; NAMED[QT.Backspace] = "Backspace"; NAMED[QT.Return] = "Enter"; NAMED[QT.Enter] = "Enter";
NAMED[QT.Insert] = "Insert"; NAMED[QT.Left] = "ArrowLeft"; NAMED[QT.Up] = "ArrowUp"; NAMED[QT.Right] = "ArrowRight"; NAMED[QT.Down] = "ArrowDown";
NAMED[QT.Tab] = "Tab";

// Qt event fields -> { key, text, ctrl, alt, shift, meta }. key is a lower-case
// letter or digit, a name from NAMED, or "" for anything else.
function fromQt(key, text, modifiers) {
    var name = NAMED[key] || "";
    if (!name && key >= 0x41 && key <= 0x5a) name = String.fromCharCode(key + 32);
    if (!name && key >= 0x30 && key <= 0x39) name = String.fromCharCode(key);
    return {
        key: name, text: text || "",
        ctrl: (modifiers & QT.Control) !== 0, alt: (modifiers & QT.Alt) !== 0,
        shift: (modifiers & QT.Shift) !== 0, meta: (modifiers & QT.Meta) !== 0,
    };
}

function printable(text) {
    return text.length > 0 && text.charCodeAt(0) >= 0x20 && text !== "\u007f";
}

// ev: fromQt(...). ctx: { view: "queue" | "library" | "jump" | "info", plOpen }.
// -> { _tag: action, ... } | null
function decide(ev, ctx) {
    var chord = ev.ctrl || ev.alt || ev.meta;
    if (ctx.plOpen && ctx.view === "jump") {
        if (ev.key === "Escape") return { _tag: "jumpClose" };
        if (ev.key === "Backspace") return { _tag: "jumpBackspace" };
        if (ev.key === "Enter" || ev.key === "Insert") return { _tag: "jumpEnter" };
        if (!chord && printable(ev.text)) return { _tag: "jumpType", text: ev.text };
        if (!chord) return { _tag: "none" };
    }
    if (ev.ctrl && !ev.alt && ev.key === "d") return { _tag: "double" };
    if (ev.ctrl && !ev.alt && ev.key === "t") return { _tag: "timeMode" };
    if (ev.alt && !ev.ctrl && ev.key === "w") return { _tag: "shade" };
    if (ev.alt && !ev.ctrl && ev.key === "g") return { _tag: "foldEq" };
    if (ev.alt && !ev.ctrl && ev.key === "e") return { _tag: "foldPl" };
    if (ev.alt && !ev.ctrl && ev.key === "3") return { _tag: "info" };
    if (chord) return null;
    var lib = ctx.plOpen && ctx.view === "library";
    switch (ev.key) {
    case "z": return { _tag: "transport", which: "prev" };
    case "x": return { _tag: "transport", which: "play" };
    case "c": return { _tag: "transport", which: "pause" };
    case "v": return { _tag: "transport", which: "stop" };
    case "b": return { _tag: "transport", which: "next" };
    case "j": return { _tag: "jump" };
    case "l": return { _tag: "library" };
    case "r": return { _tag: "repeat" };
    case "s": return { _tag: "shuffle" };
    case "Escape":
        if (ctx.plOpen && ctx.view === "info") return { _tag: "viewBack" };
        if (lib) return { _tag: "viewBack" };
        return null;
    case "Backspace": return lib ? { _tag: "viewBack" } : null;
    case "ArrowUp": return lib ? { _tag: "none" } : { _tag: "volume", delta: 0.02 };
    case "ArrowDown": return lib ? { _tag: "none" } : { _tag: "volume", delta: -0.02 };
    case "ArrowLeft": return lib ? { _tag: "viewBack" } : { _tag: "seek", deltaMs: -5000 };
    case "ArrowRight": return lib ? { _tag: "none" } : { _tag: "seek", deltaMs: 5000 };
    }
    return null;
}

// R and the repeat button cycle off -> all -> one -> off (decision 7).
function nextRepeat(mode) {
    return { off: "all", all: "one", one: "off" }[mode] || "all";
}
