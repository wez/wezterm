# `KeyAssignment` enumeration

A `KeyAssignment` represents a pre-defined function that can be applied
to control the Window, Tab, Pane state typically when a key or mouse event
is triggered.

Internally, in the underlying Rust code, `KeyAssignment` is an enum
type with a variant for each possible action known to wezterm.  In Lua,
enums get represented as a table with a single key corresponding to
the variant name.

In most cases the [`wezterm.action`](../wezterm/action.md) function is
used to create an instance of `KeyAssignment` and make it a bit more
clear and convenient.

## Available Key Assignments


