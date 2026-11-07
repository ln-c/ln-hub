#!/bin/bash
# bare.sh NAME (guest): ln-hub straight in foot (no proxy), tiled, font size 9, LN_HUB_TIMING on.
# Keys via Hyprland send_shortcut; grim shots at fixed offsets after `l`.
. /tmp/vmenv.sh
N=$1; D=/tmp/ev/$N; rm -rf $D; mkdir -p $D
pkill -x foot; pkill -x ln-hub; sleep 0.5
key() { hyprctl eval "hl.dispatch(hl.dsp.send_shortcut({mods=\"$2\", key=\"$1\", window=\"title:lnhub-$N\"}))" >/dev/null; echo "$(date +%s.%N) key $1" >> $D/bare-keys.log; }
shot() { grim $D/$1.png; echo "$(date +%s.%N) grim $1" >> $D/bare-keys.log; }
setsid -f foot -T lnhub-$N -o main.font="monospace:size=9" env LN_HUB_TIMING=$D/timing.log /tmp/ln-hub >/dev/null 2>&1
sleep 2.5
key l; sleep 0.15; shot pending-150ms; sleep 3; shot drawn-3s
key q; sleep 0.5
