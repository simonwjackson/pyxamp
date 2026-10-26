.pragma library
// Which face, and which appearance, the desktop theme asks for. Pure: the
// texts of the state files and of the registry in, the answer out.
// DesktopTheme.qml reads the files (and watches them) and calls these.
//
// The contract (docs/brief.md, "Which face"), the same files the desktop reads:
//   registry:   $PYXAMP_DESKTOP_THEMES, else $XDG_CONFIG_HOME/quickshell/mountainous/themes.json
//               (default ~/.config/...). A JSON array of entries { id, pyxampFace, ... }.
//   theme name: $XDG_STATE_HOME/mountainous/theme-name (default ~/.local/state), trimmed.
//               Resolved as mountainous's modules/quickshell/Theme.qml does: a missing file,
//               or a name that is not an id in the registry, is "platinum".
//   face:       that entry's pyxampFace, exactly "platinum" or "winamp". An entry with no
//               or an unknown pyxampFace gives the Platinum face. A registry that cannot be
//               read at all falls back to the id: "win98" is the Winamp face, anything else
//               the Platinum face.
//   appearance: .../mountainous/theme ("dark" | "light") and .../mountainous/theme-extra
//               ("on" | "off"). Extra dark applies only in dark mode, as the desktop's bar.

var FACES = ["platinum", "winamp"];
var DEFAULT_NAME = "platinum";          // Theme.qml: property string name: "platinum"
var DEFAULT_MODE = "dark";              // Theme.qml: property string mode: "dark"

function stateDir(env) {
    return env.XDG_STATE_HOME ? env.XDG_STATE_HOME : env.HOME + "/.local/state";
}
function configDir(env) {
    return env.XDG_CONFIG_HOME ? env.XDG_CONFIG_HOME : env.HOME + "/.config";
}
// env: { PYXAMP_DESKTOP_THEMES, XDG_CONFIG_HOME, XDG_STATE_HOME, HOME } (unset = "" or undefined)
function paths(env) {
    var s = stateDir(env) + "/mountainous/";
    return {
        registry: env.PYXAMP_DESKTOP_THEMES ? env.PYXAMP_DESKTOP_THEMES : configDir(env) + "/quickshell/mountainous/themes.json",
        name: s + "theme-name",
        mode: s + "theme",
        extra: s + "theme-extra",
    };
}

// The registry's text -> an array of entries, or null when it cannot be read
// (missing, not JSON, not an array). Entries that are not objects are skipped.
function parseRegistry(text) {
    if (text === null || text === undefined) return null;
    var v;
    try { v = JSON.parse(text); } catch (e) { return null; }
    if (!Array.isArray(v)) return null;
    var out = [];
    for (var i = 0; i < v.length; i++) if (v[i] && typeof v[i] === "object" && !Array.isArray(v[i])) out.push(v[i]);
    return out;
}

function find(registry, id) {
    for (var i = 0; i < registry.length; i++) if (registry[i].id === id) return registry[i];
    return null;
}

// nameText: the theme-name file's text, or null when the file is missing.
// registry: parseRegistry(...). -> the theme id the desktop draws.
function themeName(nameText, registry) {
    if (nameText === null || nameText === undefined) return DEFAULT_NAME;
    var n = String(nameText).trim();
    if (registry === null) return n === "" ? DEFAULT_NAME : n;
    return find(registry, n) ? n : DEFAULT_NAME;
}

// -> "platinum" | "winamp"
function face(nameText, registryText) {
    var registry = parseRegistry(registryText), name = themeName(nameText, registry);
    if (registry === null) return name === "win98" ? "winamp" : "platinum";
    var entry = find(registry, name);
    if (entry && FACES.indexOf(entry.pyxampFace) >= 0) return entry.pyxampFace;
    return "platinum";
}

// modeText, extraText: the files' texts, or null when missing. previousMode:
// the mode shown so far. Theme.qml keeps its mode when the file holds anything
// but "dark" or "light", and starts at "dark".
// -> { mode: "dark" | "light", extraDark, appearance: "light" | "grey" | "black" }
function appearance(modeText, extraText, previousMode) {
    var v = modeText === null || modeText === undefined ? "" : String(modeText).trim();
    var mode = v === "dark" || v === "light" ? v : (previousMode === "light" || previousMode === "dark" ? previousMode : DEFAULT_MODE);
    var extra = extraText !== null && extraText !== undefined && String(extraText).trim() === "on";
    var black = mode === "dark" && extra;
    return { mode: mode, extraDark: extra, appearance: mode === "light" ? "light" : black ? "black" : "grey" };
}
