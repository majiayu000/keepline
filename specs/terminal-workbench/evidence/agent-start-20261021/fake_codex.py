#!/usr/bin/env python3
"""Isolated stand-in for an Agent TUI: turns on bracketed paste, shows what it receives,
and marks a submit only on a bare Enter. No model, no network."""
import os, sys, termios, tty
fd = 0
old = termios.tcgetattr(fd)
tty.setraw(fd)
out = sys.stdout
out.write("\x1b[?2004h\r\n  Codex（隔离测试替身）\r\n\r\n› ")
out.flush()
pasting = False
try:
    while True:
        data = os.read(fd, 4096)
        if not data:
            break
        if b"\x1b[200~" in data:
            pasting = True
        text = data.replace(b"\x1b[200~", b"").replace(b"\x1b[201~", b"")
        if b"\x1b[201~" in data:
            pasting = False
        if not pasting and text in (b"\r", b"\n"):
            out.write("\r\n[已发送]\r\n› ")
        else:
            out.write(text.decode("utf-8", "replace").replace("\n", "\r\n  "))
        out.flush()
finally:
    termios.tcsetattr(fd, termios.TCSADRAIN, old)
