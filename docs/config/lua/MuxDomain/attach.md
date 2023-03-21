# `domain:attach()`

{{since('20230320-124340-559cb7b0')}}

Attempts to attach the domain.

Attaching a domain will attempt to import the windows, tabs and panes from the
remote system into those of the local GUI.

Unlike the [AttachDomain](../keyassignment/AttachDomain.md) key assignment,
calling `domain:attach()` will *not* implicitly spawn a new pane into the
domain if the domain contains no panes. This is to provide flexibility when
used in the [gui-startup](../gui-events/gui-startup.md) event.

If the domain is already attached, calling this method again has no effect.

See also: [domain:detach()](detach.md) and [domain:state()](state.md).
