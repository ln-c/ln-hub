#!/usr/bin/env python3
"""Transparent pty proxy run inside foot. Spawns ln-hub in a pty, forwards bytes both ways
(so foot answers the terminal queries), feeds the output into a pyte screen, timestamps events
and runs scheduled actions (grim screenshot, screen dump, wtype keys, direct key injection).
usage: proxy.py OUTDIR [--silent] [--ev WHEN:DELAYMS:ACTION:ARG ...] -- cmd args
WHEN: start | init | ready | readygone | prompt ; ACTION: grim | dump | wtype | inject | sh | key
S24 additions: `key` injects like `inject` and then watches the pyte screen (text plus colours and
reverse/bold attributes): the first output chunk that changes it is logged as
`EFFECT key ... +<ms> after send` with the changed rows (and the status row goes to rows.txt). Every output chunk that carries a sixel
start (ESC P) is logged as `SIXEL ...`. Encode windows: once a key was sent, two output chunks
< 25 ms apart with no key sent in the 40 ms before (ln-hub's 10 ms poll, used only while an encode
is in flight) open window N (`WHEN` = `fastN`, N = 1, 2, ...), logged `FAST window N open`; it closes
at the next sixel chunk or a > 60 ms gap between chunks (`FAST window N closed`). Keys still
without an effect at exit are logged `UNRESOLVED`."""
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
fast = {'n': 0, 'open': False, 'last_chunk': None, 'last_key': None}
def track_fast(n, is_sixel):
    last = fast['last_chunk']; fast['last_chunk'] = n
    if fast['last_key'] is None or last is None: return
    gap = n - last
    if fast['open']:
        if is_sixel or gap > 60:
            fast['open'] = False
            L(f'FAST window {fast["n"]} closed ({"sixel" if is_sixel else f"gap {gap:.1f} ms"})')
    elif not is_sixel and gap < 25 and n - fast['last_key'] > 40:
        fast['n'] += 1; fast['open'] = True; state[f'fast{fast["n"]}'] = n
        L(f'FAST window {fast["n"]} open (chunk gap {gap:.1f} ms, last key {n - fast["last_key"]:.1f} ms ago)')
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
    elif act == 'key':
        k = arg.encode().decode('unicode_escape')
        pending.append((k, now(), snap())); fast['last_key'] = now()
        os.write(mfd, k.encode('latin1'))
        L(f'KEY sent {k!r}')
QRE = re.compile(rb'\x1b\[[0-9;?]*c|\x1b_G.*?\x1b\\|\x1b\[1[0-9]t|\x1b\[>q|\x1b\[\?u')
def check_events():
    last = last_row[0]  # S24: refreshed after each pyte feed, not rebuilt every loop
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
_st = {'in': False, 'esc': False, 'carry': b''}
def strip_dcs(data):
    """Drop DCS (sixel) and APC (kitty) strings, which pyte would print as text.
    S24: linear state machine (the S16 version re-scanned the whole pending DCS on every chunk,
    which made the proxy, not the terminal, the bottleneck for large sixels)."""
    data = _st['carry'] + data; _st['carry'] = b''; out = bytearray(); i = 0; n = len(data)
    while i < n:
        if _st['in']:
            if _st['esc']:
                _st['esc'] = False
                if data[i:i+1] == b'\\': _st['in'] = False; i += 1; continue
            e = data.find(b'\x1b\\', i)
            if e < 0:
                _st['esc'] = data.endswith(b'\x1b'); i = n; break
            _st['in'] = False; i = e + 2
        else:
            js = [j for j in (data.find(b'\x1bP', i), data.find(b'\x1b_', i)) if j >= 0]
            if not js:
                if data.endswith(b'\x1b'): _st['carry'] = b'\x1b'; out += data[i:n-1]
                else: out += data[i:]
                break
            j = min(js); out += data[i:j]; _st['in'] = True; i = j + 2
    return bytes(out)
last_row = ['']
pending = []   # (key, sent_ms, screen signature at send)
def snap():
    return [tuple((c.data, c.fg, c.bg, c.bold, c.reverse) for c in (screen.buffer[y][x] for x in range(screen.columns)))
            for y in range(screen.lines)]
def check_effects():
    if not pending: return
    cur = snap()
    keep = []
    for k, t, before in pending:
        rows = [y for y in range(len(cur)) if y >= len(before) or cur[y] != before[y]]
        if rows:
            L(f'EFFECT key {k!r} +{now() - t:.1f} ms after send; rows changed {rows[:6]}{"..." if len(rows) > 6 else ""}; '
              f'last row |{screen.display[-1].rstrip()}|')
            with open(os.path.join(outdir, 'rows.txt'), 'a') as f:
                f.write(f'{now():8.1f} ms {"effect " + repr(k):>14} |{screen.display[-1]}|\n')
        else: keep.append((k, t, before))
    pending[:] = keep
status = None
try:
    while True:
        r, _, _ = select.select([0, mfd], [], [], 0.005)
        if mfd in r:
            try: data = os.read(mfd, 65536)
            except OSError: data = b''
            if not data: break
            raw.write(f'{now():.1f} {len(data)} ' + (repr(data[:80]) + '...' if len(data) > 200 else repr(data)) + '\n')
            if b'\x1bP' in data: L(f'SIXEL start in output chunk ({len(data)} bytes)')
            was_in = _st['in']
            text = strip_dcs(data)
            if b'\x1bP' in data: track_fast(now(), True)
            elif was_in: fast['last_chunk'] = now()   # sixel payload continuation, not a frame
            else: track_fast(now(), False)
            # S24: a pure sixel-payload chunk leaves the screen unchanged; skip pyte and the checks
            # so the proxy keeps up with the terminal write.
            if text:
                stream.feed(text)
                last_row[0] = screen.display[-1]
                check_effects()
            out = QRE.sub(b'', data) if silent else data
            if silent and out != data: L(f'stripped queries from output ({len(data)-len(out)} bytes)')
            if out: os.write(1, out)
            if text: check_events()
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
for k, t, _ in pending: L(f'UNRESOLVED key {k!r} sent at {t:.1f} ms: no screen change seen')
L(f'child exited status={status}')
if status is None:
    try: _, status = os.waitpid(pid, 0)
    except ChildProcessError: pass
    L(f'child exit status={status}')
time.sleep(0.3)
