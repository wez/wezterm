#!/bin/bash

bindgen bindings.h -o src/lib.rs \
  --no-layout-tests \
  --no-doc-comments \
  --blocklist-type "FT_(Int16|UInt16|Int32|UInt32|Int16|Int64|UInt64)" \
  --raw-line "#![allow(non_snake_case)]" \
  --raw-line "#![allow(non_camel_case_types)]" \
  --raw-line "#![allow(non_upper_case_globals)]" \
  --raw-line "#![allow(clippy::unreadable_literal)]" \
  --raw-line "#![allow(clippy::upper_case_acronyms)]" \
  --raw-line "pub type FT_Int16 = i16;" \
  --raw-line "pub type FT_UInt16 = u16;" \
  --raw-line "pub type FT_Int32 = i32;" \
  --raw-line "pub type FT_UInt32 = u32;" \
  --raw-line "pub type FT_Int64 = i64;" \
  --raw-line "pub type FT_UInt64 = u64;" \
  --default-enum-style rust \
  --generate=functions,types,vars \
  --allowlist-function="FT_.*" \
  --allowlist-type="[FT]T_.*" \
  --allowlist-var="[FT]T_.*" \
  -- -Ifreetype2/include
