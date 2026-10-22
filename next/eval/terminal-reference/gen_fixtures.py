#!/usr/bin/env python3
"""Generate deterministic terminal fixtures (raw VT byte streams) for the Ghostty comparison.

Each fixture fits in COLS x ROWS and is replayed unchanged into Ghostty and Keepline.
Usage: python3 gen_fixtures.py [out_dir]
"""

from __future__ import annotations

import sys
from pathlib import Path

COLS, ROWS = 100, 30
ESC = "\x1b"
CSI = ESC + "["
RESET = CSI + "0m"


def label(text: str) -> str:
    return f"{CSI}38;5;244m{text}{RESET}"


def grid(start: int, end: int, per_row: int, gap: str = "") -> list[str]:
    chars = [chr(c) for c in range(start, end + 1)]
    return [gap.join(chars[i : i + per_row]) for i in range(0, len(chars), per_row)]


def screen(lines: list[str], tail: str = "") -> bytes:
    # Home + clear, explicit CRLF so the stream does not depend on tty ONLCR.
    body = "\r\n".join(lines[: ROWS - 1])
    return (f"{CSI}H{CSI}2J" + body + tail).encode()


def box_blocks() -> bytes:
    lines = [label("U+2500-257F box drawing (64/row, then spaced)")]
    lines += grid(0x2500, 0x257F, 64)
    lines += grid(0x2500, 0x257F, 32, " ")[:2]
    lines.append(label("U+2580-259F blocks"))
    lines += grid(0x2580, 0x259F, 32) + grid(0x2580, 0x259F, 32, " ")
    lines.append(label("continuity: rows must touch, no gaps between cells"))
    frame = ["┌" + "─" * 18 + "┬" + "─" * 18 + "┐", "│" + " " * 18 + "│" + " " * 18 + "│",
             "├" + "═" * 18 + "╪" + "═" * 18 + "┤", "│" + "░" * 9 + "▒" * 9 + "│" + "▓" * 9 + "█" * 9 + "│",
             "╰" + "─" * 18 + "┴" + "─" * 18 + "╯"]
    blocks = ["█" * 12 + "  " + "▀▄" * 6, "█" * 12 + "  " + "▌▐" * 6, "█" * 12 + "  " + "▖▗▘▙▚▛▜▝▞▟▙▛",
              "█" * 12 + "  " + "╱╲" * 6, "█" * 12 + "  " + "╳" * 12]
    for f, b in zip(frame, blocks):
        lines.append(f + "   " + b)
    lines.append(label("U+2800-28FF braille (64/row)"))
    lines += grid(0x2800, 0x28FF, 64)
    lines.append(label("powerline E0A0-E0A3 E0B0-E0BF + segments"))
    pl = [chr(c) for c in list(range(0xE0A0, 0xE0A4)) + list(range(0xE0B0, 0xE0C0))]
    lines.append(" ".join(pl))
    seg = (f"{CSI}48;5;25;38;5;231m main {CSI}38;5;25;48;5;238m{CSI}38;5;231m  feat "
           f"{CSI}38;5;238;48;5;64m{CSI}38;5;231m ok {RESET}{CSI}38;5;64m{RESET}"
           f"  {CSI}38;5;238m{CSI}48;5;238;38;5;231m 12:00 {CSI}38;5;25m{CSI}48;5;25;38;5;231m 100% {RESET}")
    lines.append(seg)
    return screen(lines)


def text_unicode() -> bytes:
    lines = [label("CJK (each glyph = 2 cells; the ruler below must stay aligned)")]
    lines.append("0123456789" * 6)
    lines.append("中文显示测试：终端渲染对照，宽字符必须占两格。日本語のテキスト。한국어 텍스트")
    lines.append("全角标点：，。！？（）【】「」 ｱｲｳ半角カナ ABCＡＢＣ")
    lines.append("|中|文|对|齐|  |ab|cd|ef|gh|")
    lines.append(label("emoji: single / VS16 / ZWJ / flags / skin tone / keycap"))
    lines.append("😀 🚀 ✅ ❌ ⚠️ ❤️ ☕ 👍🏽 👨‍👩‍👧‍👦 🏳️‍🌈 🇨🇳 🇺🇸 1️⃣ #️⃣ 🧑‍💻")
    lines.append("|😀|🚀|✅|❌|☕|")
    lines.append(label("combining marks: e+0301 a+0308 n+0303 o+031B+0323 stacked; Hangul jamo; Thai; Devanagari"))
    lines.append("é ä ñ ợ Z̵̷̸ x⃝ q̣̇ | 각 | กิ่ | क्ष")
    lines.append("café naïve résumé Ångström Øresund")
    lines.append(label("ligature candidates (should stay one glyph per cell): -> => != === <= >= ::"))
    lines.append("-> => != === <= >= :: ... www |> <|> /* */ fi fl")
    lines.append(label("ascii reference"))
    lines.append("The quick brown fox jumps over the lazy dog 0123456789 !@#$%^&*()_+-=[]{};':\",./<>?")
    lines.append(f"{CSI}1mBold: The quick brown fox 0123456789{RESET}  {CSI}3mItalic: The quick brown fox{RESET}")
    lines.append(label("wide char at line end must wrap, not split:"))
    lines.append("x" * (COLS - 1) + "汉" + "后续")
    return screen(lines)


def sgr_colors() -> bytes:
    lines = [label("SGR styles")]
    styles = [("1", "bold"), ("2", "dim"), ("3", "italic"), ("4", "underline"), ("4:2", "double"),
              ("21", "dbl(21)"), ("4:3", "curly"), ("4:4", "dotted"), ("4:5", "dashed"), ("5", "blink"),
              ("7", "inverse"), ("8", "hidden"), ("9", "strike"), ("53", "overline")]
    lines.append(" ".join(f"{CSI}{c}m{n}{RESET}" for c, n in styles[:7]))
    lines.append(" ".join(f"{CSI}{c}m{n}{RESET}" for c, n in styles[7:]))
    lines.append(f"{CSI}4:3;58:2::255:80:80mred curly{RESET} {CSI}4;58;5;45mcyan underline{RESET} "
                 f"{CSI}1;3;4;9mall{RESET} {CSI}2;3mdim italic{RESET} {CSI}7;31minv red{RESET} {CSI}1;7mbold inv{RESET}")
    lines.append(label("16 colors fg (normal / bright) and bg"))
    lines.append("".join(f"{CSI}{30 + i}m fg{i:<2}{RESET}" for i in range(8))
                 + "".join(f"{CSI}{90 + i}m fg{i + 8:<2}{RESET}" for i in range(8)))
    lines.append("".join(f"{CSI}{40 + i};97m bg{i:<2}{RESET}" for i in range(8))
                 + "".join(f"{CSI}{100 + i};30m bg{i + 8:<2}{RESET}" for i in range(8)))
    lines.append(f"{CSI}1m" + "".join(f"{CSI}{30 + i}m bold{i}" for i in range(8)) + RESET + "  (bold-as-bright check)")
    lines.append(label("256-color palette"))
    lines.append("".join(f"{CSI}48;5;{i}m   " for i in range(16)) + RESET)
    for base in range(16, 232, 36):
        lines.append("".join(f"{CSI}48;5;{i}m  " for i in range(base, base + 36)) + RESET)
    lines.append("".join(f"{CSI}48;5;{i}m    " for i in range(232, 256)) + RESET)
    lines.append(label("truecolor gradients"))
    lines.append("".join(f"{CSI}48;2;{int(255 * x / 99)};0;{255 - int(255 * x / 99)}m " for x in range(COLS)) + RESET)
    lines.append("".join(f"{CSI}48;2;0;{int(255 * x / 99)};80m " for x in range(COLS)) + RESET)
    lines.append("".join(f"{CSI}38;2;255;{int(255 * x / 99)};0m█" for x in range(COLS)) + RESET)
    return screen(lines)


def cursor(shape: int, name: str) -> bytes:
    lines = [label(f"DECSCUSR {shape} ({name}); cursor parked after the arrow ->")]
    lines.append("")
    lines.append("prompt> echo cursor-shape ")
    return screen(lines, tail=f"{CSI}3;27H{CSI}{shape} q")


def hyperlinks() -> bytes:
    osc = lambda url, text: f"{ESC}]8;;{url}{ESC}\\{text}{ESC}]8;;{ESC}\\"  # noqa: E731
    lines = [label("OSC 8 hyperlinks (Ghostty underlines on hover and opens with cmd-click)")]
    lines.append("plain " + osc("https://example.invalid/a", "link-one") + " plain " + osc("file:///tmp", "文件链接"))
    lines.append("bare URL (auto-detect): https://ghostty.org/docs  path: /usr/local/bin")
    return screen(lines)


FIXTURES = {
    "box-blocks": box_blocks,
    "text-unicode": text_unicode,
    "sgr-colors": sgr_colors,
    "cursor-block": lambda: cursor(2, "steady block"),
    "cursor-underline": lambda: cursor(4, "steady underline"),
    "cursor-bar": lambda: cursor(6, "steady bar"),
    "hyperlinks": hyperlinks,
}


def main() -> int:
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent / "fixtures"
    out.mkdir(parents=True, exist_ok=True)
    for name, make in FIXTURES.items():
        (out / f"{name}.vt").write_bytes(make())
        print(f"fixture {name}.vt")
    return 0


if __name__ == "__main__":
    sys.exit(main())
