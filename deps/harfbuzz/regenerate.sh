#!/bin/bash

bindgen bindings.h -o src/lib.rs \
  --no-layout-tests \
  --no-doc-comments \
  --raw-line "#![allow(non_snake_case)]" \
  --raw-line "#![allow(non_camel_case_types)]" \
  --raw-line "#![allow(non_upper_case_globals)]" \
  --raw-line "#![allow(clippy::unreadable_literal)]" \
  --default-enum-style rust \
  --generate=functions,types,vars \
  --whitelist-function="hb_.*" \
  --whitelist-type="hb_.*" \
  -- -Iharfbuzz/src -I../freetype/freetype2/include
