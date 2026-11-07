#!/bin/bash
cd /tmp/vmwork; N=$1; GEO=$2; FS=$3
args=(--ev readygone:200:inject:l --ev readygone:700:inject:G --ev readygone:1500:inject:'\r')
for i in 1 2 3 4 5 6 7 8 9 10 11 12; do args+=(--ev readygone:$((1500+i*120)):row:after-enter-$i); done
args+=(--ev readygone:1700:grim:job-status --ev readygone:1700:dump:job-status --ev readygone:4500:row:late --ev readygone:4500:grim:job-late --ev readygone:5000:sh:'pkill -x ln-hub')
./vm.sh run $N $GEO $FS "${args[@]}" -- /tmp/ln-hub
