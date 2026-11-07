#!/bin/bash
# starth.sh (guest): start Hyprland from ssh on seatd
export XDG_RUNTIME_DIR=/run/user/1000 LIBSEAT_BACKEND=seatd
setsid -f Hyprland > /tmp/hypr.log 2>&1 < /dev/null
