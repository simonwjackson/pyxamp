# One built-in face per desktop theme

Approved by Simon on 2026-10-25, after the Platinum face mock (`/tmp/macamp-mock/`, temporary; key shots kept in `docs/assets/platinum-face/`). This ADR supersedes the "one" in ADR 0002. ADR 0002's "no skin support" stands.

## Context

ADR 0002 gave Pyxamp one redrawn Winamp 2 skin. Simon's desktop now has two themes in mountainous: `platinum` (Mac OS 8) and `win98`. The Winamp face looks right on the Windows 98 desktop. It looks foreign on the Platinum desktop, where the bar, the menus and the windoids all use Platinum tokens and ChicagoFLF. The mock showed that the approved behaviour can wear a Platinum face with no change to the state model: its Platinum renderer drew every state of groups 1–8 from the unchanged model files, and 61 of 61 interaction checks passed (verified in the mock's `REPORT.md`; not yet tested in Quickshell).

## Decision

- Pyxamp has two built-in faces: the Winamp face (approved in mock passes 1 and 2) and the Platinum face. Both are renderers of one state model. Neither face owns state, and the daemon owns playback (ADR 0003), so a face switch keeps all state and playback.
- Both faces are first-party and compiled in. Pyxamp still loads no skin files: no `.wsz`, no skin loader, no user files.
- The face follows the desktop theme. The face reads the `theme-name` state file (`$XDG_STATE_HOME/mountainous/theme-name`, the file that mountainous's `Theme.qml` reads):
  - `platinum` gives the Platinum face;
  - `win98` or any unknown theme gives the Winamp face.
- The preferred mechanism is data, not an id check: a `pyxampFace` field on each theme entry in mountainous's `modules/quickshell/themes.json`. A new theme then picks a face without a Pyxamp change. This is a cross-repo dependency. The change belongs in mountainous and is not part of this decision's commit.
- The face's appearance (light, dark grey, extra dark) follows the desktop's `theme` and `theme-extra` state files, as the bar does.
- There is no face override. The O menu gets no "Face" item.
- The product name stays Pyxamp. Code, settings and visible strings say "Platinum face" and never use "MacAMP" or "Mac". Two reasons: "Mac" is an Apple mark, and Apple restricts it in third-party product names (assumed, not checked against Apple's current guidelines); and "MacAMP" was a real product of Nullsoft, @soft and Subband, whose name a fan site and other projects still use (`docs/assets/platinum-face/REFERENCES.md`).

## Consequences

- Each desktop theme gets a native-looking player, and Simon learns one set of keys and behaviours. The display's colour keeps one meaning on both faces: amber means "following another player".
- A face may change how a behaviour looks, but it may not add or remove a behaviour. The Platinum face maps each Mac control to an existing action (zoom box = Ctrl+D, collapse box = windowshade, close box = hide).
- A theme switch changes the face at once. No restart, no lost place.
- Until mountainous ships `pyxampFace`, the face must fall back to the id check above.

## Costs

- Two art sets. The Winamp face has 10 hand-drawn sprite sheets. The Platinum face draws procedurally (Platinum primitives, a seven-segment digit, 1-bit ChicagoFLF), with no bitmaps.
- Two renderers on one state model. In the mock that is 542 + 700 lines.
- Every new control, menu and state must be designed twice, once for each face.
- The boards and the checks run twice, once for each face.
- Two fit plans, because the faces have different sizes (275 × 435 sp and 300 × 515 sp).
- Hidden cost: drag mapping. A drag must map from each control's own rectangle, not from hard-coded Winamp coordinates. The mock's `input.js` used `sx - 114`, and the Platinum renderer had to add its own `macDragUpdate`. The production face has the same problem today: `face/MainSection.qml` maps the volume drag with `(x + mx - 114) / 54` and the seek drag with `(x + mx - 30) / 219`. Both faces must map drags from each control's rectangle before the Platinum face is built.

## Considered Options

- Keep one face (ADR 0002 as written): no extra work, but Pyxamp looks foreign on the Platinum desktop.
- Load `.wsz` skins and ship a Platinum skin: it reuses one renderer, but it reopens skin support. A `.wsz` skin only replaces bitmaps in Winamp's fixed layout, so it cannot give a disclosure header, a Finder list view or the desktop's menus.
- A face override in the O menu: more control, but one more setting and one more state to test. Simon chose to follow the desktop only.
