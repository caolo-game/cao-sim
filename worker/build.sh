#/usr/bin/bash

set -e

cargo install --path . --root . --no-default-features --features=jemallocator
