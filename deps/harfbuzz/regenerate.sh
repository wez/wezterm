#!/bin/bash

bindgen bindings.h -o src/lib.rs \
  --no-layout-tests \
  --no-doc-comments \
  --raw-line "#![allow(non_snake_case)]" \
  --raw-line "#![allow(non_camel_case_types)]" \
  --raw-line "#![allow(non_upper_case_globals)]" \
  --raw-line "#![allow(clippy::unreadable_literal)]" \
  --raw-line "#![allow(clippy::upper_case_acronyms)]" \
  --default-enum-style rust \
  --generate=functions,types,vars \
  --allowlist-function="hb_.*" \
  --allowlist-type="hb_.*" \
  -- -Iharfbuzz/src -I../freetype/freetype2/include
