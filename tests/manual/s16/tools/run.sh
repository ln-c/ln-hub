#!/bin/bash
# usage: run.sh NAME WxH_PX|tiled FONTSIZE proxyargs...   (runs inside the VM; returns once the app has started)
. /tmp/vmenv.sh
pkill -x foot; pkill -x ln-hub; pkill -f proxy.py; sleep 0.5
NAME=$1; GEO=$2; FS=$3; shift 3; [ "$FS" = default ] && FS=
rm -rf /tmp/ev/$NAME; mkdir -p /tmp/ev/$NAME
foot -T lnhub-$NAME ${FS:+-o main.font="monospace:size=$FS"} -e python3 /tmp/proxy.py /tmp/ev/$NAME "$@" > /tmp/ev/$NAME.foot.log 2>&1 &
sleep 1.2
if [ "$GEO" != tiled ]; then
  W=${GEO%x*}; H=${GEO#*x}
  hyprctl dispatch "hl.dsp.window.float({action=\"enable\"})" >/dev/null
  hyprctl dispatch "hl.dsp.window.resize({x=$W,y=$H,exact=true})" >/dev/null
  hyprctl dispatch "hl.dsp.window.center()" >/dev/null
  sleep 0.5
fi
touch /tmp/ev/$NAME/go
