#!/bin/bash
# shots.sh NAME (host): what the preview shows while the first hover's image is pending.
# grim right after `l` (decode in flight), at the start of encode window 1, and 2.5 s later.
cd "$(dirname "$0")"; N=$1
./vm.sh run $N tiled 9 --ev readygone:200:key:l \
 --ev readygone:230:grim:pending-decode --ev readygone:230:dump:pending-decode \
 --ev fast1:0:grim:pending-encode --ev fast1:0:dump:pending-encode \
 --ev readygone:2700:grim:drawn --ev readygone:2700:dump:drawn --ev readygone:3500:sh:'pkill -x ln-hub' \
 -- env LN_HUB_TIMING=/tmp/ev/$N/timing.log /tmp/ln-hub
