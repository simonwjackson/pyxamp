.pragma library
// The Platinum face's colours, per appearance: "light", "grey" (dark grey) and
// "black" (extra dark). Pure data.
//
// The chrome values are the desktop's own Platinum tokens, copied from
// mountainous modules/quickshell/PlatinumTokens.qml (cairn branch, read
// 2026-10-26): Pyxamp is its own Quickshell config (qs -n -c pyxamp, ADR 0005)
// and cannot import that file. Only the LCD values and the label chips are new
// (the approved mock, /tmp/macamp-mock/mac.js APPEAR; docs/brief.md tokens).
// Extra dark is the bar's black: a flat face with #5C5C5C/#8C8C8C outlines and
// no bevels. Menus and balloons stay dark grey there, as the bar's menus do.

var GREY_MENU = {
    menu: "#3a3a3a", menuText: "#ececec", menuText2: "#b8b8b8", menuOff: "#767676", menuEdge: "#000000",
    menuHi: "#565656", menuLo: "#1e1e1e", menuShadow: "#9e000000", menuSel: "#5858d0", balloon: "#2c2c2c", balloonEdge: "#000000", balloonText: "#ececec",
};

var APPEARANCES = {
    light: {
        flat: false, face: "#dddddd", hi: "#ffffff", lo: "#999999", edge: "#000000", btnEdge: "#000000", raised: "#e6e6e6", well: "#bdbdbd", press: "#a8a8a8",
        text: "#000000", text2: "#3a3a3a", off: "#8a8a8a", sel: "#333399", selText: "#ffffff", list: "#ffffff", sorted: "#eeeeee", headSorted: "#c8c8c8",
        stripeHi: "#ffffff", stripeLo: "#999999", glyph: "#000000", lampOn: "#2fbf2f", lampOnHi: "#acdfac", lampOff: "#7a7a7a", fill: "#333399", fillHi: "#6666cc",
        ridge: "#999999", danger: "#9e2525", field: "#ffffff", focusRing: "#9999ff", discl: "#8a8a8a", groove: "#bdbdbd", star: "#a8801e", starHi: "#f8e8a0",
        menu: "#dddddd", menuText: "#000000", menuText2: "#3a3a3a", menuOff: "#8a8a8a", menuEdge: "#000000", menuHi: "#ffffff", menuLo: "#999999", menuShadow: "#222222",
        menuSel: "#333399", balloon: "#ffffff", balloonEdge: "#000000", balloonText: "#000000",
        labels: { pyxis: "#e07020", jellyfin: "#3a6ad0", ossicle: "#2a9a3a" },
        lcd: { bg: "#b4c0a2", ghost: "#a9b697", ink: "#1c2616", mid: "#4a5840", dim: "#8d9b7e", err: "#a01a0c" },
        lcdF: { bg: "#e9b24a", ghost: "#dea640", ink: "#2a1904", mid: "#60400e", dim: "#b98a32", err: "#8a1408" },
        speed: "#2a9aa2",
    },
    grey: {
        flat: false, face: "#3a3a3a", hi: "#565656", lo: "#1e1e1e", edge: "#000000", btnEdge: "#000000", raised: "#444444", well: "#242424", press: "#2a2a2a",
        text: "#ececec", text2: "#b8b8b8", off: "#767676", sel: "#5858d0", selText: "#ffffff", list: "#2c2c2c", sorted: "#333333", headSorted: "#2e2e2e",
        stripeHi: "#565656", stripeLo: "#1e1e1e", glyph: "#ececec", lampOn: "#48d848", lampOnHi: "#b6efb6", lampOff: "#262626", fill: "#5858d0", fillHi: "#8686e6",
        ridge: "#1e1e1e", danger: "#ef8f8f", field: "#242424", focusRing: "#7c7ce8", discl: "#6a6a6a", groove: "#242424", star: "#a8801e", starHi: "#f8e8a0",
        labels: { pyxis: "#e08840", jellyfin: "#6a94f0", ossicle: "#4ab85a" },
        lcd: { bg: "#0a0e09", ghost: "#121911", ink: "#a6e89c", mid: "#5f9a58", dim: "#2c462a", err: "#ff6a58" },
        lcdF: { bg: "#0e0a04", ghost: "#1f1607", ink: "#ffb020", mid: "#b47a14", dim: "#4c3410", err: "#ff6a58" },
        speed: "#2a9aa2",
    },
    black: {
        flat: true, face: "#000000", hi: "", lo: "", edge: "#5c5c5c", btnEdge: "#8c8c8c", raised: "#000000", well: "#262626", press: "#262626",
        text: "#e6e6e6", text2: "#a4a4a4", off: "#5c5c5c", sel: "#5858d0", selText: "#ffffff", list: "#000000", sorted: "#121212", headSorted: "#1a1a1a",
        stripeHi: "#3a3a3a", stripeLo: "", glyph: "#e6e6e6", lampOn: "#48d848", lampOnHi: "#b6efb6", lampOff: "#262626", fill: "#5858d0", fillHi: "#8686e6",
        ridge: "#3a3a3a", danger: "#e07a7a", field: "#000000", focusRing: "#7c7ce8", discl: "#6a6a6a", groove: "#000000", star: "#e0b850", starHi: "#f8e8a0",
        labels: { pyxis: "#e08840", jellyfin: "#6a94f0", ossicle: "#4ab85a" },
        lcd: { bg: "#000000", ghost: "#0d130c", ink: "#a6e89c", mid: "#5f9a58", dim: "#24391f", err: "#ff6a58" },
        lcdF: { bg: "#000000", ghost: "#191206", ink: "#ffb020", mid: "#b47a14", dim: "#4c3410", err: "#ff6a58" },
        speed: "#2a9aa2",
    },
};
(function () {
    // Menus and balloons: grey and black take the desktop's dark grey menus.
    for (var k in GREY_MENU) { if (APPEARANCES.grey[k] === undefined) APPEARANCES.grey[k] = GREY_MENU[k]; APPEARANCES.black[k] = GREY_MENU[k]; }
})();

var NAMES = ["light", "grey", "black"];
var LCD_ROLES = ["ink", "mid", "ghost", "dim", "err"];

function palette(name) { return APPEARANCES[name] || APPEARANCES.light; }
// The LCD set: own audio (sage or green) or following another player (amber).
function lcd(name, following) { var p = palette(name); return following ? p.lcdF : p.lcd; }

// Every colour LCD text is drawn in, once each (tools/gen-lcd-font makes one row per colour).
function lcdInks() {
    var out = [];
    for (var i = 0; i < NAMES.length; i++) for (var f = 0; f < 2; f++) {
        var set = lcd(NAMES[i], f === 1);
        for (var r = 0; r < LCD_ROLES.length; r++) if (out.indexOf(set[LCD_ROLES[r]]) < 0) out.push(set[LCD_ROLES[r]]);
    }
    return out;
}
