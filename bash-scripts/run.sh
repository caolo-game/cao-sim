#/usr/bin/bash

set -e

./diesel migration run
./caolo-worker
