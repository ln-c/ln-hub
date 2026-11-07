#!/bin/bash
cd /tmp/vmwork; N=$1; GEO=$2; FS=$3
args=(--ev readygone:200:inject:l --ev readygone:500:inject:/ --ev readygone:700:inject:wp_small --ev readygone:900:inject:'\r' --ev readygone:1600:row:before-apply --ev readygone:1700:inject:'\r')
for i in 1 2 3 4 5 6 7 8 9 10 11 12; do args+=(--ev readygone:$((1700+i*100)):row:after-enter-$i); done
args+=(--ev readygone:1900:grim:job-status --ev readygone:1900:dump:job-status --ev readygone:5000:row:late --ev readygone:5000:grim:job-late --ev readygone:5400:sh:'pkill -x ln-hub')
./vm.sh run $N $GEO $FS "${args[@]}" -- /tmp/ln-hub
