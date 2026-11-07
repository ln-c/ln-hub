#!/bin/bash
# burst.sh NAME (host): S24 key burst in foot 176x43 (tiled, font size 9), LN_HUB_TIMING on.
# `l` focuses the Theme items (cursor on wp_large_1.jpg, a fresh 6000x4000 jpg). In the first encode
# window (item 1's encode in flight) `?` then `j` 20 ms later (hover -> wp_large_2.jpg); in the next
# encode window (wp_large_2's encode in flight) `k` (hover back to wp_large_1).
cd "$(dirname "$0")"; N=$1
./vm.sh run $N tiled 9 --ev readygone:200:key:l \
 --ev fast1:0:key:'?' --ev fast1:20:key:j --ev fast2:0:key:k \
 --ev fast2:1500:dump:final --ev fast2:1500:grim:final --ev fast2:2500:sh:'pkill -x ln-hub' \
 -- env LN_HUB_TIMING=/tmp/ev/$N/timing.log /tmp/ln-hub
