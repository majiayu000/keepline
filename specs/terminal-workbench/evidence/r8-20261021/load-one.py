"""Identical 10 Hz, 80x20 alternate-screen producer for both terminal apps."""
import os,signal,time
active=False
running=True
frame=0
def toggle(*_):
 global active
 active=not active
def stop(*_):
 global running
 running=False
signal.signal(signal.SIGUSR1,toggle)
signal.signal(signal.SIGTERM,stop)
os.write(1,b'\x1b[?1049h\x1b[?25l\x1b[2J\x1b[HREADY: 10 Hz / 80 x 20 / alternate screen\r\n')
try:
 while running:
  if not active:signal.pause();continue
  frame+=1
  rows=[f'{frame:08} row {row:02} '+('0123456789abcdef'*4)[:60] for row in range(20)]
  os.write(1,('\x1b[H'+'\r\n'.join(rows)).encode())
  time.sleep(.1)
finally:os.write(1,b'\x1b[?25h\x1b[?1049l')
