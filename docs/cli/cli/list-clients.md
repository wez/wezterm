# `wezterm cli list-clients`

*Run `wezterm cli list-clients --help` to see more help*

Lists the set of connected clients and some additional information about them:

```
$ wezterm cli list-clients
USER HOST     PID CONNECTED     IDLE       WORKSPACE FOCUS
wez  foo  1098536 166.03140978s 31.40978ms default       0
```

The meanings of the fields are:

* `USER` - the username associated with the session
* `HOST` - the hostname associated with the session
* `PID` - the process id of the client session
* `CONNECTED` - shows how long the connection has been established
* `IDLE` - shows how long it has been since input was received from that client
* `WORKSPACE` - shows the active workspace for that session
* `FOCUS` - shows the pane id of the pane that has focus in that session

{{since('20220624-141144-bd1b7c5d')}}

You may request JSON output:

```
$ wezterm cli list-clients --format json
[
  {
    "username": "wez",
    "hostname": "foo",
    "pid": 1098536,
    "connection_elapsed": {
      "secs": 226,
      "nanos": 502667166
    },
    "idle_time": {
      "secs": 0,
      "nanos": 502667166
    },
    "workspace": "default",
    "focused_pane_id": 0
  }
]
```

## Synopsis

```console
{% include "../../examples/cmd-synopsis-wezterm-cli-list-clients--help.txt" %}
```
