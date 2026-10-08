"""Re-create terminal-ancestry.png from the asciinema recording.

Needs agg (https://github.com/asciinema/agg), Pillow, and the JetBrains Mono font.

    curl -sL -o oflh.cast https://asciinema.org/a/1266562.cast
    python3 .github/social-preview/terminal-frame.py oflh.cast
"""

import json
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

FRAME = 64  # 16 locked files, FileLockExample selected, ancestry panel filled
FONT_SIZE, LINE_HEIGHT = 32, 1.3
PAD_X, PAD_Y = 19, 21  # agg's outer padding at this font size
CELL_W, CELL_H = FONT_SIZE * 0.6, FONT_SIZE * LINE_HEIGHT
PANEL_COL, FIRST_ROW, LAST_ROW = 144, 7, 18  # details panel: title to last ancestry line

cast = Path(sys.argv[1])
out = Path(__file__).with_name("terminal-ancestry.png")
lines = cast.read_text().splitlines()
kept, outputs = [lines[0]], 0
for line in lines[1:]:
    kept.append(line)
    if json.loads(line)[1] == "o":
        outputs += 1
        if outputs == FRAME:
            break
kept.append(json.dumps([1.0, "o", ""]))

with tempfile.TemporaryDirectory() as tmp:
    trimmed, gif = Path(tmp, "frame.cast"), Path(tmp, "frame.gif")
    trimmed.write_text("\n".join(kept) + "\n")
    subprocess.run(
        ["agg", "--font-family", "JetBrains Mono", "--font-size", str(FONT_SIZE),
         "--line-height", str(LINE_HEIGHT), "--idle-time-limit", "1", "--no-loop",
         str(trimmed), str(gif)],
        check=True,
    )
    im = Image.open(gif)
    im.seek(im.n_frames - 1)
    frame = im.convert("RGB")

box = (
    int(PAD_X + PANEL_COL * CELL_W - 44),
    int(PAD_Y + FIRST_ROW * CELL_H - 26),
    frame.width,
    int(PAD_Y + LAST_ROW * CELL_H + 16),
)
frame.crop(box).save(out, optimize=True)
print("wrote", out)
