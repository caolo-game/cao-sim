#!/usr/bin/bash

./diesel migration run

set -e

./caolo-worker
