"""Draw the background of the macOS disk-image window.

The picture is committed at src-tauri/dmg/background.png and this script is how
it was made, so that a later session can change a word without redrawing it by
hand and without wondering what font it was.

    python tools/mac_dmg_background.py

What it draws, and why those words. The reader of the .dmg is somebody who has
been sent a link by their pastor and has never installed an unsigned Mac app.
The window they open has to do two things at once: show them the drag, and warn
them -- before they double-click -- that macOS is about to stop them. So the
icons sit on the top half, an arrow between them, and three short lines sit
underneath:

    This app is free and unsigned.
    Your Mac will warn you the first time you open it.
    Open READ ME FIRST for the exact steps.

The third line is the important one and points at READ-ME-FIRST.rtf, which sits
in the same window and carries the actual steps. **The RTF is the authoritative
copy, not this picture.** Text baked into an image cannot be read by a screen
reader, cannot be copied, and cannot be corrected by anybody who is not running
this script; and the Finder layout that positions all of it needs a window
server and may not run on a build machine at all. If any of that fails, the file
is still in the image and still says everything. tools/make_dmg.sh says which
happened.

Nobody has seen this rendered inside a Finder window. The image itself has been
looked at; how macOS draws it has not.

The window is 660 x 560 points and the picture is the same size in pixels: one
image pixel per point, which is what Finder assumes. On a Retina display the
text is therefore drawn at 1x and will be a little soft, which is why it is set
large. The icon positions in make_dmg.sh must match the gaps left here.
"""

import io
import os
import sys

from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
OUT = os.path.join(ROOT, 'src-tauri', 'dmg', 'background.png')

W, H = 660, 560

PAPER = (247, 245, 240)
INK = (43, 43, 43)
QUIET = (110, 106, 100)
RULE = (222, 217, 208)

# Where make_dmg.sh puts the three icons. Drawn here only as the arrow between
# the first two; the icons themselves are Finder's.
APP_XY = (170, 150)
APPLICATIONS_XY = (490, 150)
README_XY = (330, 300)

LINES = [
    ('This app is free and unsigned.', True),
    ('Your Mac will warn you the first time you open it.', False),
    ('Open READ ME FIRST for the exact steps.', True),
]

# Fonts are looked for by file, because PIL's default bitmap font is unreadable
# at this size. The first that exists wins. Windows first: this was drawn on the
# build machine, which is a Windows one.
FONT_CANDIDATES = {
    'regular': [
        r'C:\Windows\Fonts\segoeui.ttf',
        r'C:\Windows\Fonts\arial.ttf',
        '/System/Library/Fonts/Helvetica.ttc',
        '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',
    ],
    'bold': [
        r'C:\Windows\Fonts\segoeuib.ttf',
        r'C:\Windows\Fonts\arialbd.ttf',
        '/System/Library/Fonts/Helvetica.ttc',
        '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf',
    ],
}


def font(kind, size):
    for path in FONT_CANDIDATES[kind]:
        if os.path.exists(path):
            return ImageFont.truetype(path, size)
    raise SystemExit(
        'no usable font found. Add one to FONT_CANDIDATES; the committed '
        'src-tauri/dmg/background.png was drawn with Segoe UI.')


def centred(draw, y, text, f, fill):
    w = draw.textbbox((0, 0), text, font=f)[2]
    draw.text(((W - w) / 2.0, y), text, font=f, fill=fill)


def arrow(draw, x0, x1, y):
    """A plain arrow from the app icon to the Applications folder."""
    draw.line([(x0, y), (x1 - 16, y)], fill=QUIET, width=4)
    draw.polygon(
        [(x1, y), (x1 - 20, y - 11), (x1 - 20, y + 11)],
        fill=QUIET,
    )


def main():
    img = Image.new('RGB', (W, H), PAPER)
    d = ImageDraw.Draw(img)

    # The arrow sits between the two icons, on their centre line. Finder draws
    # 128-point icons, so it starts and stops clear of them.
    arrow(d, APP_XY[0] + 78, APPLICATIONS_XY[0] - 78, APP_XY[1])

    # A rule under the icon half, so the words below read as a separate thing
    # and not as a caption on the folder.
    rule_y = README_XY[1] + 90
    d.line([(70, rule_y), (W - 70, rule_y)], fill=RULE, width=1)

    big = font('bold', 24)
    med = font('regular', 21)
    y = rule_y + 26
    for text, strong in LINES:
        f = big if strong else med
        centred(d, y, text, f, INK if strong else QUIET)
        y += 40 if strong else 38

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    img.save(OUT, 'PNG', optimize=True)
    print('wrote %s (%d x %d, %d bytes)' % (OUT, W, H, os.path.getsize(OUT)))
    print('icon positions for make_dmg.sh: app %s  applications %s  readme %s'
          % (APP_XY, APPLICATIONS_XY, README_XY))
    return 0


if __name__ == '__main__':
    sys.exit(main())
