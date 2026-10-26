# One built-in face per desktop theme

Approved by Simon on 2026-10-25, after the Platinum face mock (`/tmp/macamp-mock/`, temporary; key shots kept in `docs/assets/platinum-face/`). This ADR supersedes the "one" in ADR 0002. ADR 0002's "no skin support" stands.

## Context

ADR 0002 gave Pyxamp one redrawn Winamp 2 skin. Simon's desktop now has two themes in mountainous: `platinum` (Mac OS 8) and `win98`. The Winamp face looks right on the Windows 98 desktop. It looks foreign on the Platinum desktop, where the bar, the menus and the windoids all use Platinum tokens and ChicagoFLF. The mock showed that the approved behaviour can wear a Platinum face with no change to the state model: its Platinum renderer drew every state of groups 1–8 from the unchanged model files, and 61 of 61 interaction checks passed (verified in the mock's `REPORT.md`; not yet tested in Quickshell).

## Decision

- Pyxamp has two built-in faces: the Winamp face (approved in mock passes 1 and 2) and the Platinum face. Both are renderers of one state model. Neither face owns state, and the daemon owns playback (ADR 0003), so a face switch keeps all state and playback.
- Both faces are first-party and compiled in. Pyxamp still loads no skin files: no `.wsz`, no skin loader, no user files.
- The face follows the desktop theme. The mechanism is data: the `pyxampFace` field (`"platinum"` or `"winamp"`) of the theme's entry in mountainous's theme registry (`modules/quickshell/themes.json`). The theme name comes from the `theme-name` state file, resolved as mountainous's `Theme.qml` resolves it, so a missing file or an unknown name is `platinum`. An entry with no or an unknown `pyxampFace` gives the Platinum face. Only when the registry cannot be read does the id decide (`win98` is the Winamp face). The exact rule, with its paths, is in `docs/brief.md` ("Which face").
  - *Amended 2026-10-26.* This bullet first said that a present but unknown theme name gives the Winamp face, and that the `pyxampFace` field was a preferred mechanism to come, with an id check until then. Both changed with the build of the Platinum face: an unknown name resolves to `platinum`, as the desktop draws it, and `pyxampFace` is the mechanism (mountainous adds the field).
- The face's appearance (light, dark grey, extra dark) follows the desktop's `theme` and `theme-extra` state files, as the bar does.
- There is no face override. The O menu gets no "Face" item.
- The product name stays Pyxamp. Code, settings and visible strings say "Platinum face" and never use "MacAMP" or "Mac". Two reasons: "Mac" is an Apple mark, and Apple restricts it in third-party product names (assumed, not checked against Apple's current guidelines); and "MacAMP" was a real product of Nullsoft, @soft and Subband, whose name a fan site and other projects still use (`docs/assets/platinum-face/REFERENCES.md`).

## Consequences

- Each desktop theme gets a native-looking player, and Simon learns one set of keys and behaviours. The display's colour keeps one meaning on both faces: amber means "following another player".
- A face may change how a behaviour looks, but it may not add or remove a behaviour. The Platinum face maps each Mac control to an existing action (zoom box = Ctrl+D, collapse box = windowshade, close box = hide).
- A theme switch changes the face at once. No restart, no lost place.
- A registry that cannot be read falls back to the id (`win98` is the Winamp face). *Amended 2026-10-26: this replaces "until mountainous ships `pyxampFace`, the face must fall back to the id check above".*

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
