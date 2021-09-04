# Multiple

*Since: nightly builds only*

Performs a sequence of multiple assignments.  This is useful when you
want a single key press to trigger multiple actions.

The example below causes `LeftArrow` to effectively type `left`:

```lua
return {
  keys = {
    {key="LeftArrow", action={Multiple={
      {SendKey={key="l"}},
      {SendKey={key="e"}},
      {SendKey={key="f"}},
      {SendKey={key="t"}},
    }}}
  }
}
```
