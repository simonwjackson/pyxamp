import QtQuick
import Quickshell
import Quickshell.Io

// Pyxamp's face: its own Quickshell config (qs -c pyxamp), never part of the
// desktop's shell (ADR 0005). It draws the daemon's state and sends commands;
// the daemon owns MPRIS, playback and capture. No second MPRIS state owner here.
ShellRoot {
    id: root

    MonoClock { id: clock }
    Prefs { id: prefs }
    DaemonLink { id: link; clock: clock }
    Face { id: face; link: link; prefs: prefs; clock: clock }

    // qs -c pyxamp ipc call pyxamp toggle   (SUPER+Z)
    IpcHandler {
        target: "pyxamp"
        function toggle(): void { face.toggle(); }
        function show(): void { if (!face.p.shown) face.toggle(); }
        function hide(): void { face.hide(); }
        // A read-only report of what the face shows, for checks and bug reports.
        function describe(): string {
            return JSON.stringify({
                face: face.faceKind, appearance: face.appearance, themeName: face.themeName,
                shown: face.p.shown, screen: face.screen ? face.screen.name : "", layer: face.p.onTop ? "top" : "bottom",
                k: face.fit.k, plN: face.fit.plN, refused: face.planFor(true).refused, physW: face.fit.physW, physH: face.fit.physH,
                logicalW: face.fit.logicalW, logicalH: face.fit.logicalH, x: face.place.x, y: face.place.y,
                dbl: face.p.dbl, shade: face.p.shade, eqOpen: face.p.eqOpen, plOpen: face.p.plOpen, view: face.view, menu: face.menu,
                link: link.link._tag, lcd: face.m.lcd, status: face.m.status, marquee: face.m.marquee.text, visual: face.visual._tag,
                reducedMotion: face.p.reducedMotion, jump: face.jumpText,
                positionMs: face.m.positionMs, clock: face.m.shortClock, clockMs: face.clock.now(),
                target: face.m.target, volume: face.m.volume, volumeReal: face.m.volumeReal, muted: face.m.muted,
                volumeOn: face.m.controls.volume.enabled, volumeTip: face.m.volumeTip,
                // Step 2: what is shown and the playlist section's rows, as drawn.
                mode: face.m.mode, source: face.m.source, spoken: face.m.spoken, console: face.m.console, speed: face.m.speed,
                eqNote: face.m.eqNote, balance: face.p.balance, albumMode: face.p.albumMode, albumOpen: face.albumOpen, sel: face.sel,
                plTitle: face.pl.title, plHead: face.pl.head ? face.pl.head.l + (face.pl.head.r ? " | " + face.pl.head.r : "") : "",
                plFoot: face.pl.foot ? face.pl.foot.l : "", plSel: face.pl.sel,
                plRows: face.pl.rows.slice(0, 60).map((r) => (r.tag ? r.tag + " " : "") + (r.l || "") + (r.r ? " | " + r.r : "") + (r.chips ? " [" + r.chips.join("] [") + "]" : "")),
                plCount: face.pl.rows.length, prompt: face.prompt ? face.prompt.text : null, jumpStatus: face.jump.status,
                libPath: face.lib.path.map((e) => e.label),
                plRowY: face.pl.rows.slice(0, 60).map((r, i) => face.plRowY(i)),
                menuItems: face.menu ? face.menuItems(face.menu).filter((i) => !i.sep).map((i) => i.l + (i.dis ? " (off)" : "")) : [],
            });
        }
    }
}
