#!/bin/sh
set -x
export RUST_BACKTRACE=1
cargo run -- start --front-end null -- python -B ./ci/esctest/esctest/esctest.py \
  --expected-terminal=xterm \
  --xterm-checksum=334 \
  --v=3 \
  --timeout=0.1 \
  --no-print-logs \
  --logfile=esctest.log

#--stop-on-failure \
