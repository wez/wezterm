# `treat_left_ctrlalt_as_altgr = false`

*since: nightly*

If you are using a layout with an *AltGr* key, you may experience issues
when running inside a VNC session, because VNC emulates the AltGr keypresses
by sending plain *Ctrl-Alt* keys, which won't be understood as AltGr.

To fix this behavior you can tell WezTerm to treat left *Ctrl-Alt* keys as
*AltGr* with the option `treat_left_ctrlalt_as_altgr`. Note that the key
bindings using separate Ctrl and Alt won't be triggered anymore.

```lua
return {
  treat_left_ctrlalt_as_altgr = true
}
```
