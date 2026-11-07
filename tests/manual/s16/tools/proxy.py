#!/usr/bin/env python3
"""Transparent pty proxy run inside foot. Spawns ln-hub in a pty, forwards bytes both ways
(so foot answers the terminal queries), feeds the output into a pyte screen, timestamps events
and runs scheduled actions (grim screenshot, screen dump, wtype keys, direct key injection).
usage: proxy.py OUTDIR [--silent] [--ev WHEN:DELAYMS:ACTION:ARG ...] -- cmd args
WHEN: start | init | ready | readygone | prompt ; ACTION: grim | dump | wtype | inject | sh"""
import os, sys, pty, select, time, termios, tty, fcntl, struct, signal, subprocess, re
import pyte

args = sys.argv[1:]
outdir = args.pop(0); os.makedirs(outdir, exist_ok=True)
silent = False; evs = []
while args and args[0] != '--':
    a = args.pop(0)
    if a == '--silent': silent = True
    elif a == '--ev':
        w, d, act, arg = args.pop(0).split(':', 3); evs.append([w, float(d), act, arg, None])
args.pop(0)
cmd = args

while not os.path.exists(os.path.join(outdir, 'go')): time.sleep(0.02)
time.sleep(0.3)
def winsz(fd):
    return struct.unpack('HHHH', fcntl.ioctl(fd, termios.TIOCGWINSZ, b'\0'*8))
rows, cols, _, _ = winsz(0)
pid, mfd = pty.fork()
if pid == 0:
    os.execvp(cmd[0], cmd)
fcntl.ioctl(mfd, termios.TIOCSWINSZ, struct.pack('HHHH', rows, cols, 0, 0))
def winch(*_):
    r, c, _, _ = winsz(0)
    fcntl.ioctl(mfd, termios.TIOCSWINSZ, struct.pack('HHHH', r, c, 0, 0))
    screen.resize(r, c)
signal.signal(signal.SIGWINCH, winch)
screen = pyte.Screen(cols, rows); stream = pyte.ByteStream(screen)
old = termios.tcgetattr(0); tty.setraw(0)
t0 = time.monotonic()
def now(): return (time.monotonic() - t0) * 1000
log = open(os.path.join(outdir, 'timeline.log'), 'w')
def L(msg): log.write(f'{now():8.1f} ms  {msg}\n'); log.flush()
L(f'spawn cmd={cmd} winsize={rows}x{cols} silent={silent}')
raw = open(os.path.join(outdir, 'output.raw.log'), 'w')
state = {'init': None, 'ready': None, 'readygone': None, 'prompt': None, 'start': 0.0}
def fire(ev):
    w, d, act, arg, _ = ev
    L(f'ACTION {act} {arg!r} (after {w}+{d}ms)')
    env = os.environ
    if act == 'grim':
        subprocess.Popen(['grim', os.path.join(outdir, arg + '.png')], env=env)
    elif act == 'dump':
        with open(os.path.join(outdir, arg + '.txt'), 'w') as f:
            f.write(f'# t={now():.1f} ms  size={screen.lines}x{screen.columns}\n')
            for i, line in enumerate(screen.display): f.write(f'{i:2d}|{line}|\n')
    elif act == 'row':
        with open(os.path.join(outdir, 'rows.txt'), 'a') as f:
            f.write(f'{now():8.1f} ms {arg:>14} |{screen.display[-1]}|\n')
    elif act == 'wtype':
        subprocess.Popen(['wtype'] + arg.split(','), env=env)
    elif act == 'inject':
        os.write(mfd, arg.encode().decode('unicode_escape').encode('latin1'))
    elif act == 'sh':
        subprocess.Popen(arg, shell=True, env=env)
QRE = re.compile(rb'\x1b\[[0-9;?]*c|\x1b_G.*?\x1b\\|\x1b\[1[0-9]t|\x1b\[>q|\x1b\[\?u')
def check_events():
    last = screen.display[-1] if screen.display else ''
    n = now()
    if state['init'] is None and 'Initializing…' in last:
        state['init'] = n; L('EVENT init: status row shows Initializing…')
    if state['ready'] is None and 'Ready' in last:
        state['ready'] = n; L('EVENT ready: status row shows Ready')
    if state['ready'] is not None and state['readygone'] is None and 'Ready' not in last:
        state['readygone'] = n; L(f'EVENT readygone: Ready cleared ({n - state["ready"]:.0f} ms after Ready)')
    if state['prompt'] is None and last.startswith('/'):
        state['prompt'] = n; L('EVENT prompt')
    for ev in evs:
        if ev[4] is None and state.get(ev[0]) is not None:
            ev[4] = state[ev[0]] + ev[1]
    for ev in evs:
        if ev[4] is not None and n >= ev[4] and ev[2] != 'DONE':
            fire(ev); ev[2] = 'DONE'
_buf = [b'', False]
def strip_dcs(data):
    """Drop DCS (sixel) and APC (kitty) strings, which pyte would print as text."""
    data = _buf[0] + data; _buf[0] = b''; out = b''
    while data:
        i = min([j for j in (data.find(b'\x1bP'), data.find(b'\x1b_')) if j >= 0] or [-1]) if True else -1
        if i < 0:
            if data.endswith(b'\x1b'): _buf[0] = b'\x1b'; data = data[:-1]
            out += data; break
        out += data[:i]; data = data[i:]
        e = data.find(b'\x1b\\')
        if e < 0: _buf[0] = data; break
        data = data[e+2:]
    return out
status = None
try:
    while True:
        r, _, _ = select.select([0, mfd], [], [], 0.005)
        if mfd in r:
            try: data = os.read(mfd, 65536)
            except OSError: data = b''
            if not data: break
            raw.write(f'{now():.1f} {len(data)} {data!r}\n'); raw.flush()
            stream.feed(strip_dcs(data))
            out = QRE.sub(b'', data) if silent else data
            if silent and out != data: L(f'stripped queries from output ({len(data)-len(out)} bytes)')
            if out: os.write(1, out)
            check_events()
        if 0 in r:
            d = os.read(0, 4096)
            if d:
                L(f'stdin->child {d!r}'); os.write(mfd, d)
        check_events()
        done, st = os.waitpid(pid, os.WNOHANG)
        if done:
            status = st; break
finally:
    termios.tcsetattr(0, termios.TCSADRAIN, old)
L(f'child exited status={status}')
if status is None:
    try: _, status = os.waitpid(pid, 0)
    except ChildProcessError: pass
    L(f'child exit status={status}')
time.sleep(0.3)
