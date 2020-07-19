#/usr/bin/bash

set -e

# valgrind --tool=memcheck --leak-check=full --show-leak-kinds=all -s ./caolo-worker
valgrind --tool=massif --detailed-freq=1000 --pages-as-heap=yes --time-unit=ms --massif-out-file="./massif/massif-out" ./caolo-worker
ms_print ./massif/massif-out > ./massif/print

# heaptrack ./caolo-worker
# heaptrack_print -f "/caolo/heaptrack.caolo-worker.6.gz" > ./massif/heaptrack-print
