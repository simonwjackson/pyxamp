# One redrawn default skin, and no skin support

Pyxamp looks like Winamp 2's default skin, but every sprite is redrawn by hand in the same style. It ships none of Nullsoft's base-skin bitmaps (copyright of Winamp Group SA) and it does not load `.wsz` skins. Redrawing keeps the repository publishable and lets new controls (speed, skip, the source switcher) look native, because they are drawn into the same sheets. The cost is pixel-art work for each sprite sheet, and the loss of the 65,000 community skins.

## Amended 2026-10-25

The "one" in this decision is superseded by ADR 0006 ("One built-in face per desktop theme"): Pyxamp now has two built-in faces, the Winamp face and the Platinum face. The "no skin support" part stands. Pyxamp loads no `.wsz` skins and no skin files of any kind.
