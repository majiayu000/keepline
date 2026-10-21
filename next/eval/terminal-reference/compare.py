# /// script
# requires-python = ">=3.10"
# dependencies = ["pillow>=10"]
# ///
"""Compare Ghostty and Keepline captures of the same fixture.

For each name present in both out/ghostty and out/keepline:
  - derive the cell size from image size / grid (both captures are exactly the grid),
  - write out/compare/<name>.png: Ghostty | Keepline | diff (Keepline resampled onto
    Ghostty's grid so glyph-level differences line up; the cell-ratio gap is reported
    separately and is not hidden by the resample),
  - write out/report.json and out/report.md with numbers.
Usage: uv run compare.py [--cols 100 --rows 30]
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageOps

HERE = Path(__file__).resolve().parent


def cell(img: Image.Image, cols: int, rows: int) -> tuple[float, float]:
    return img.width / cols, img.height / rows


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cols", type=int, default=100)
    ap.add_argument("--rows", type=int, default=30)
    ap.add_argument("--out", type=Path, default=HERE / "out")
    a = ap.parse_args()
    gdir, kdir, cdir = a.out / "ghostty", a.out / "keepline", a.out / "compare"
    cdir.mkdir(parents=True, exist_ok=True)
    report = []
    for gpath in sorted(gdir.glob("*.png")):
        kpath = kdir / gpath.name
        if not kpath.exists():
            continue
        g = Image.open(gpath).convert("RGB")
        k = Image.open(kpath).convert("RGB")
        if g.width % a.cols or g.height % a.rows:
            print(f"WARNING {gpath.name}: Ghostty capture {g.size} is not a whole {a.cols}x{a.rows} grid")
        gw, gh = cell(g, a.cols, a.rows)
        if gpath.stem == "cursor-block":
            # Cursor parked at row 3, col 27 (gen_fixtures.cursor). Unfocused Ghostty draws it hollow.
            centre = g.getpixel((int(26.5 * gw), int(2.5 * gh)))
            if sum(centre) < 300:
                print("WARNING Ghostty capture was unfocused: cursor shapes are not comparable")
        kw, kh = cell(k, a.cols, a.rows)
        kr = k.resize(g.size, Image.Resampling.LANCZOS)
        diff = ImageChops.difference(g, kr).convert("L")
        hist = diff.histogram()
        total = g.width * g.height
        changed = sum(hist[33:]) / total
        mean = sum(i * n for i, n in enumerate(hist)) / total / 255
        # Per text row: which grid rows differ most (helps locate the gap in the report).
        rows = []
        for r in range(a.rows):
            band = diff.crop((0, round(r * gh), g.width, round((r + 1) * gh)))
            h = band.histogram()
            rows.append(round(sum(h[33:]) / (band.width * band.height), 4))
        heat = ImageOps.colorize(ImageOps.autocontrast(diff), black="#000000", white="#ff3355")
        pad, title = 12, 28
        sheet = Image.new("RGB", (g.width + k.width + heat.width + 4 * pad, max(g.height, k.height) + title + pad), "#202020")
        d = ImageDraw.Draw(sheet)
        x = pad
        for label, im in (("Ghostty 1.3.1", g), ("Keepline (screen.rs)", k), ("diff (Keepline resampled to Ghostty grid)", heat)):
            d.text((x, 8), f"{label}  {im.width}x{im.height}", fill="#e0e0e0")
            sheet.paste(im, (x, title))
            x += im.width + pad
        sheet.save(cdir / gpath.name)
        entry = {
            "fixture": gpath.stem,
            "ghostty_cell_px": [round(gw, 3), round(gh, 3)],
            "keepline_cell_px": [round(kw, 3), round(kh, 3)],
            "ghostty_ratio_h_w": round(gh / gw, 4),
            "keepline_ratio_h_w": round(kh / kw, 4),
            "ratio_delta": round((kh / kw) / (gh / gw) - 1, 4),
            "changed_pixels": round(changed, 4),
            "mean_abs_diff": round(mean, 4),
            "ghostty_bg": g.getpixel((g.width - 2, g.height - 2)),
            "keepline_bg": k.getpixel((k.width - 2, k.height - 2)),
            "worst_rows": sorted(range(a.rows), key=lambda r: -rows[r])[:5],
            "row_changed": rows,
        }
        report.append(entry)
        print(f"{entry['fixture']:<18} cell G {gw:.2f}x{gh:.2f} K {kw:.2f}x{kh:.2f} "
              f"ratio Δ {entry['ratio_delta']:+.1%}  changed {changed:.1%}  worst rows {entry['worst_rows']}")
    (a.out / "report.json").write_text(json.dumps(report, indent=1))
    lines = ["| fixture | Ghostty cell (px@2x) | Keepline cell | h/w Δ | changed px | worst rows |", "|---|---|---|---|---|---|"]
    for e in report:
        lines.append(f"| {e['fixture']} | {e['ghostty_cell_px'][0]}×{e['ghostty_cell_px'][1]} | "
                     f"{e['keepline_cell_px'][0]}×{e['keepline_cell_px'][1]} | {e['ratio_delta']:+.1%} | "
                     f"{e['changed_pixels']:.1%} | {e['worst_rows']} |")
    (a.out / "report.md").write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
