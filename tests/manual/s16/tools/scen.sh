#!/bin/bash
# scen.sh help NAME GEO FS   |  scen.sh prompt NAME GEO FS
cd /tmp/vmwork
kind=$1; N=$2; GEO=$3; FS=$4
K='pkill -x ln-hub'
if [ $kind = help ]; then
 ./vm.sh run $N $GEO $FS --ev readygone:100:dump:a-modules-left-empty --ev readygone:100:grim:a-modules-left-empty \
  --ev readygone:200:inject:l --ev readygone:500:dump:b-items-left-empty --ev readygone:500:grim:b-items-left-empty \
  --ev readygone:700:inject:'?' --ev readygone:1000:dump:c-help --ev readygone:1000:grim:c-help \
  --ev readygone:1300:sh:"$K" -- /tmp/ln-hub
elif [ $kind = prompt ]; then
 args=(--ev readygone:200:inject:l --ev readygone:400:row:items-empty --ev readygone:500:inject:/ --ev readygone:700:row:prompt-/)
 t=800; tot=0
 for n in 5 10 5 4 1 1 1 1 1 1 1 1 1 1 5 10; do
   tot=$((tot+n)); s=$(printf 'a%.0s' $(seq $n))
   args+=(--ev readygone:$t:inject:"$s" --ev readygone:$((t+120)):row:typed-$tot --ev readygone:$((t+120)):grim:p$tot); t=$((t+250))
 done
 args+=(--ev readygone:$((t+100)):sh:"$K")
 ./vm.sh run $N $GEO $FS "${args[@]}" -- /tmp/ln-hub
fi
sleep $(( kind=="x" ? 0 : 1 ))
