# ActivateKeyTable

*Since: nightly builds only*

Activates a named key table.

See [Key Tables](../../key-tables.md) for a detailed example.

The following parameters are possible:

* `name` - the name of the table to activate.  The name must match up to an entry in the `key_tables` configuration.
* `timeout_milliseconds` - an optional duration expressed in milliseconds. If specified, then the activation will automatically expire and pop itself from the key table stack once that duration elapses.  If omitted, this activation will not expire due to time.
* `one_shot` - an optional boolean that controls whether the activation will pop itself after a single additional key press.  The default if left unspecified is `one_shot=true`. When set to `false`, pressing a key will not automatically pop the activation and you will need to use either a timeout or an explicit key assignment that triggers [PopKeyTable](PopKeyTable.md) to cancel the activation.
* `replace_current` - an optional boolean. If set to true then behave as though [PopKeyTable](PopKeyTable.md) was triggered before pushing this new activation on the stack.  This is most useful for key assignments in a table that was activated using `one_shot=false`.
