# Nop

Causes the key press to have no effect; it behaves as though those
keys were not pressed.

```lua
return {
  keys = {
    -- Turn off any side effects from pressing CMD-m
    {key="m", mods="CMD", action="Nop"},
  }
}
```


