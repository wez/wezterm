#!/bin/bash

bindgen bindings.h -o src/types.rs \
  --no-layout-tests \
  --no-doc-comments \
  --blocklist-type "FT_(Int16|UInt16|Int32|UInt32|Int16|Int64|UInt64)" \
  --raw-line "#![allow(non_camel_case_types)]" \
  --default-enum-style rust \
  --generate=types \
  --allowlist-type="FT_(Fixed|Pos|F\\d+Dot\\d+)" \
  -- -Ifreetype2/include

bindgen bindings.h -o src/lib.rs \
  --no-layout-tests \
  --no-doc-comments \
  --blocklist-type "FT_(Int16|UInt16|Int32|UInt32|Int16|Int64|UInt64)" \
  --raw-line "#![allow(non_snake_case)]" \
  --raw-line "#![allow(non_camel_case_types)]" \
  --raw-line "#![allow(non_upper_case_globals)]" \
  --raw-line "#![allow(clippy::unreadable_literal)]" \
  --raw-line "#![allow(clippy::upper_case_acronyms)]" \
  --raw-line "mod types;" \
  --raw-line "mod fixed_point;" \
  --raw-line "pub use fixed_point::*;" \
  --raw-line "pub type FT_Int16 = i16;" \
  --raw-line "pub type FT_UInt16 = u16;" \
  --raw-line "pub type FT_Int32 = i32;" \
  --raw-line "pub type FT_UInt32 = u32;" \
  --raw-line "pub type FT_Int64 = i64;" \
  --raw-line "pub type FT_UInt64 = u64;" \
  --default-enum-style rust \
  --generate=functions,types,vars \
  --allowlist-function="(SVG|FT)_.*" \
  --allowlist-type="(SVG|[FT]T)_.*" \
  --allowlist-var="(SVG|[FT]T)_.*" \
  -- -Ifreetype2/include

perl -i -pe 's,^pub type FT_Fixed =,//$&,' src/lib.rs
perl -i -pe 's,^pub type FT_F26Dot6 =,//$&,' src/lib.rs
perl -i -pe 's,^pub type FT_F2Dot14 =,//$&,' src/lib.rs
perl -i -pe 's,^pub type FT_Pos =,//$&,' src/lib.rs
