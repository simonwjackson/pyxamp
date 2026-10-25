# References: Platinum-era Mac players

Researched 2026-10-24. I had no `web_search` tool in this run, so I fetched the pages with `curl`: DuckDuckGo's HTML results (it blocked me after a few queries), Wikipedia, the Wayback Machine (archived macamp.com, 1998 to 2001), Panic and Macworld. I looked at five screenshots, but I did not copy them into the mock. They sat in `/tmp/macamp-ref/`, outside the served folder, and I deleted them after I wrote this file. No pixel of a reference is in any page, sprite or shot.

Labels: **verified** means I read it on the page cited. **Seen** means I looked at the screenshot, and the description is mine.

## Correction to the brief (read first)

The brief says MacAMP was "by @soft, later folded into Winamp for Mac". The sources say something different:

- **verified:** MacAmp was first released on 13 April 1997 by Dmitry Boldyrev of Nullsoft, later of Subband. Its MP3 decoding came from PlayMedia's AMP decoder. In September 1999 it was revamped and renamed "Macast". https://en.wikipedia.org/wiki/MacAmp
- **verified:** In February 1999, the MacAMP FAQ said "@soft is the official developer of MacAMP". https://web.archive.org/web/19990203161751/http://macamp.com:80/faq.shtml
- **verified:** A 2002 OS X version, "MacAMP Lite X", came from Subband Technologies. https://github.com/RetroSoftwareRepository/OpenAMP-Lite-X
- **Not supported:** "folded into Winamp for Mac". In 2000, Nullsoft/AOL asked Panic to make Audion its Mac player, not MacAMP (https://panic.com/extras/audionstory/). The only Winamp for Mac was the 2011 "Winamp Sync for Mac" beta (https://en.wikipedia.org/wiki/Winamp). I found no source that MacAMP became part of Winamp.

## The finding that shaped the design

**verified:** The real MacAMP did not look Platinum. Cabel Sasser of Panic wrote that MacAMP was "very nice and efficient, but, with the exception of the name, not very 'Mac' — it even shared the same skins with WinAMP." That is Panic's view, and Panic was a rival. https://panic.com/extras/audionstory/

So a 1998 MacAMP user remembers a hi-fi or Winamp-like skin, not Platinum chrome. Of the apps I checked, the one that looks most like a Platinum audio player is Apple's own AppleCD Audio Player. The face therefore takes the following from each source:

- **From AppleCD Audio Player:** the chrome, the LCD in a bevel, and the disclosure triangle.
- **From SoundJam:** the Finder-style list.
- **From MacAMP:** the name and the segmented-LCD feel.

## Per player

| | MacAMP (1997–1999) | Audion (Panic, 1999) | SoundJam MP (Casady & Greene, 1999) | AppleCD Audio Player (Mac OS 8) |
|---|---|---|---|---|
| Window chrome | **Seen**, 1999 1.0 screenshot: a dark gunmetal hi-fi chassis, no Platinum title bar. Its own window, with "macamp" printed small at the bottom right [1]. **verified**, 1999 notes: "Totally new GUI", "EQs are now in the separate window", "Minimized view" [2] | **verified:** non-rectangular "faces" with alpha channels and soft drop shadows, made by compositing a screen capture [3]. Many later faces copied OS X pinstripes and brushed metal [4] | **verified:** the player "evokes the look of Apple's QuickTime Player … the same brushed-aluminum" [5]. **Seen** [6]: the playlist is a separate window with a Platinum title bar (pinstripes, close, zoom, collapse) | **Seen** [7]: a Platinum window with a pinstriped title bar and a close box, a grow box, and a disclosure triangle that shows the track list |
| Display | **Seen** [1]: a cyan dot-matrix LCD with a seven-segment clock ("03:13"), the file name, and small BIT/BAL/VOL segment meters. **Seen** [8], an optional skin: a black LCD, a green clock, and "[ 44khz, 128 kbit, stereo ]" | **verified:** one face "displayed track info on a glossy LCD with scanlines" [4]. **Seen** [9]: green, teal and blue LCD faces with "00:00" seven-segment clocks | **Seen** [6]: a light grey-green LCD with dark text: the title, the time, a small spectrum, then "44 kHz" and "128 kbps" in boxes | **Seen** [7]: a black LCD with yellow seven-segment digits (Track 03, Elapsed Time 01:03), and green-on-black track times in the list |
| Buttons | **Seen** [1]: flat rectangular transport keys in a row. **Seen** [10], MacAmp 2.0 (2001): round silver keys and a volume knob | **verified:** each button has its own image when pressed, disabled or hovered [11] | **Seen** [6]: round transport keys and "Rpt." / "Shuffle" / "PL" / "EQ" pills. **verified:** a horizontal volume slider ("instead of an awkward virtual knob") [5] | **Seen** [7]: square Platinum bevel buttons, NORMAL/SHUFFLE/PROG with small indicator lights, and a vertical volume slider |
| Spectrum | **Seen** [1]: cyan segmented bars. **verified**, 2.0: "our hard-coded spectral analyzer, with cool built-in visuals" [10] | Not found | **Seen** [6]: a small spectrum in the LCD. **verified:** three bundled visual plug-ins, such as Melt-O-Rama [5] | None (CD player) |
| Skins | **verified:** "Skin is a special file containing GUI elements that replaces standard MacAMP look". The FAQ adds: "We don't want to limit the imagination with WinAMP GUI restrictions … there is a 3rd party converter … that converts WinAMP skins to MacAMP skins", and custom layouts "need Metrowerks Constructor" [12] | **verified:** a face is a base image plus images for button states, indicators and animations, kept in resource forks [11]. In 2021, Panic converted 867 faces [4] | **verified:** 13 bundled skins ("most … are pig-ugly"), more for download, and MacAmp skins can be converted [5]. **verified:** it later read Audion faces too [3] | None |
| What made it feel Mac | Little, says Panic [3]. Its 1999 page sells "Elegant and usable user interface" and "Easy GUI customization" [2] | Craft and the alpha-channel shadows. Panic aimed to be "the first truly Mac-like solution for playing MP3s" [3] | Familiar Apple QuickTime 4 styling, and drag-and-drop between multiple playlists [5] | It was the system's own Platinum: the same bevels, triangle, grow box and Chicago as the Finder |

Other facts used:
- **verified:** By 1999, MacAmp was called "the most popular MP3 player for the Macintosh". https://en.wikipedia.org/wiki/MacAmp
- **verified:** SoundJam MP came out on 14 July 1999 and became the basis of iTunes 1.0. https://en.wikipedia.org/wiki/SoundJam_MP
- **verified:** Audion 1.0 came out on 16 August 1999. https://panic.com/extras/audionstory/

## Sources

1. MacAMP 1.0 screenshot, archived 1999-04-29: https://web.archive.org/web/19990429040600im_/http://www.macamp.com:80/images/mashot.gif
2. MacAMP description page, archived 1999-02-21: https://web.archive.org/web/19990221101747/http://www.macamp.com:80/description.shtml
3. Cabel Sasser, "The True Story of Audion", Panic: https://panic.com/extras/audionstory/
4. Panic Blog, "Facing Forward" (2021): https://panic.com/blog/facing-forward/
5. Jim Heid, "SoundJam MP" review, Macworld (1999): https://www.macworld.com/article/159570/soundjam.html
6. SoundJam MP Free screenshot (non-free image on Wikipedia): https://en.wikipedia.org/wiki/File:Soundjammp.jpg
7. "Apple CD Player" image in Panic's Audion story: https://panic.com/extras/audionstory/popup-applecd.html
8. "One of the Cool Optional Skins" (MacAMP 1.0 Preview), mywinamp.com: http://www.mywinamp.com/macamp-player/
9. "Audion Faces" image in Panic's Audion story: https://panic.com/extras/audionstory/popup-audionfaces.html
10. MacAmp 2.0 GUI page, archived 2001-09-11: https://web.archive.org/web/20010911024048/http://www.macamp.com:80/features/gui.html
11. Panic Blog, "Saving Face" (2020): https://panic.com/blog/saving-face/
12. MacAMP FAQ, archived 1999-02-03: https://web.archive.org/web/19990203161751/http://macamp.com:80/faq.shtml

## Naming risk (flag)

- **"MacAMP" was a real product name.** It belonged to Nullsoft, @soft and Subband [Wikipedia; 12]. A fan site still uses it (https://macamp.nu/), and so does "OpenAMP Lite X" (GitHub, above). "Winamp" is a trademark of Nullsoft/AOL. mywinamp.com's own disclaimer says so: http://www.mywinamp.com/macamp-player/
- **"Mac" is an Apple mark.** Apple restricts it in third-party product names. I did not check Apple's current trademark guidelines, so treat this as a known risk, not a verified rule.
- **What the mock does:** no visible string says "MacAMP" or "Mac". The window title is "Pyxamp". "MacAMP" appears only in mock-control labels and documents. In code and settings, call it the **Platinum face** (the theme id is already `platinum` in `themes.json`).
- **Look-alike risk:**
  - The disclosure triangles, title-bar boxes and grow box are generic shapes in the Mac OS 8 manner. I drew them on a 1-sp grid; I did not trace them.
  - ChicagoFLF is a public-domain-by-statement outline derived from Susan Kare's Chicago (see `/tmp/platinum-mock/FONTS-2.md`).
  - For a personal desktop, I judge the risk as low (opinion). Before anything is published, have someone qualified review this.
