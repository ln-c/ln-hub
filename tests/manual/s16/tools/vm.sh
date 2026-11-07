#!/bin/bash
# vm.sh run NAME GEO FS proxyargs...  | vm.sh fetch NAME | vm.sh sh CMD
K=/home/ln/git/dotfiles/.agents/skills/arch-vm-tester/vm_data/id_ed25519
O="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -i $K"
case $1 in
 run) shift; ssh $O -p 2222 ln@localhost "bash /tmp/run.sh $(printf "%q " "$@")";;
 fetch) mkdir -p /tmp/vmwork/ev/$2; scp -q $O -P 2222 "ln@localhost:/tmp/ev/$2/*" /tmp/vmwork/ev/$2/;;
 sh) shift; ssh $O -p 2222 ln@localhost ". /tmp/vmenv.sh; $*";;
esac
