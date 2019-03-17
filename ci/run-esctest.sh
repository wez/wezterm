#!/bin/sh
set -x
export RUST_BACKTRACE=1
cargo run -- start --front-end null -- python -B ./ci/esctest/esctest/esctest.py \
  --expected-terminal=xterm \
  --v=3 \
  --timeout=0.1 \
  --logfile=esctest.log
