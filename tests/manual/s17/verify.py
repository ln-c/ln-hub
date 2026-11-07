#!/usr/bin/env python3 -I
"""Checks the S17 status-row rules against the screen dumps/rows captured in the VM."""
import glob, os, re, sys
F = "Font: Terminess Nerd Font Propo, size 15"
H = "h/j/k/l move  gg/G top/bottom  ⏎ apply  / filter  f font  s size  ? help  q quit"
assert len(F) == 40 and len(H) == 80
ev = sys.argv[1]
def lastrow(path):
    lines = [l.rstrip('\n') for l in open(path, encoding='utf-8') if l[:2].strip().isdigit() or re.match(r'\d+\|', l)]
    m = re.match(r'\s*\d+\|(.*)\|$', lines[-1]); return m.group(1)
def check(width, left, row, label):
    """left = left text (str); rule: F whole+right-aligned iff len(left)+1+40<=width, else no F cell at all."""
    assert len(row) == width, (label, len(row), width)
    if left == "":
        want = F[-width:] if width >= 40 else F[:width]
        want = want.rjust(width) if width >= 40 else want
    elif len(left) + 1 + 40 <= width:
        want = left + " " * (width - len(left) - 40) + F
    else:
        want = left[:width].ljust(width)
    ok = row == want
    print(f"{'PASS' if ok else 'FAIL'}  W={width:3d} {label:32s} |{row.rstrip() if width<100 else row.rstrip()[:110]+'…'}|")
    return ok
allok = True
for name in sorted(os.listdir(ev)):
    if not name.startswith('help-'): continue
    w = int(re.search(r'winsize=\d+x(\d+)', open(f'{ev}/{name}/timeline.log').read()).group(1))
    r = lastrow(f'{ev}/{name}/a-modules-left-empty.txt'); ok = r.strip() == ""; allok &= ok
    print(f"{'PASS' if ok else 'FAIL'}  W={w:3d} {'modules focus, empty row (no F)':32s} blank")
    allok &= check(w, "", lastrow(f'{ev}/{name}/b-items-left-empty.txt'), 'items focus, left empty')
    allok &= check(w, H, lastrow(f'{ev}/{name}/c-help.txt'), 'items focus, ? help open')
for name, w in (('pr80', 80), ('pr44', 44)):
    for line in open(f'{ev}/{name}/rows.txt', encoding='utf-8'):
        m = re.match(r'\s*[\d.]+ ms\s+(\S+) \|(.*)\|$', line.rstrip('\n'))
        lab, row = m.group(1), m.group(2)
        if lab.startswith('typed-') or lab == 'prompt-/':
            n = int(lab.split('-')[1]) if lab.startswith('typed-') else 0
            prompt = 'Filter: ' + 'a' * n if lab != 'prompt-/' else 'Filter: '
            prompt = prompt.rstrip() if lab == 'prompt-/' else prompt
            allok &= check(w, prompt, row, f'prompt {lab}')
print('ALL PASS' if allok else 'SOME FAIL')
