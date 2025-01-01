---
tags:
  - serial
---
# `serial_ports = {}`

{{since('20230408-112425-69ae8472')}}

Define a list of serial port(s) that you use regularly.
Each entry defines a `SerialDomain` with the following fields:

* `name` - the name to use for the serial domain. Must be unique across
  all multiplexer domains in your configuration.
* `port` - the name of the serial device. On Windows systems this can be
  a name like `COM0`. On Posix systems this will be a device path something
  like `/dev/ttyUSB0`.  If omitted, the `name` field be interpreted as
  the port name.
* `baud` - the communication speed to assign to the port. If omitted,
  the default baud rate will be 9600.

This configuration defines a single port:

```lua
config.serial_ports = {
  {
    name = '/dev/tty.usbserial-10',
    baud = 115200,
  },
}
```

You can then use the port in one of the following ways:

* `wezterm connect /dev/tty.usbserial-10` - this behaves similarly to `wezterm
  serial /dev/tty.usbserial-10 --baud 115200`.
* Start wezterm normally, then use the Command Palette or Launcher Menu to
  spawn a new tab in the `/dev/tty.usbserial-10` domain to connect to the
  serial device
* You can reference the serial domain by its name `/dev/tty.usbserial-10` in
  the various tab/window spawning key assignments that include a
  [SpawnCommand](../SpawnCommand.md)

You can define multiple ports if you require, and use friendly name for them:

```lua
config.serial_ports = {
  {
    name = 'Sensor 1',
    port = '/dev/tty.usbserial-10',
    baud = 115200,
  },
  {
    name = 'Sensor 2',
    port = '/dev/tty.usbserial-11',
    baud = 115200,
  },
}
```
