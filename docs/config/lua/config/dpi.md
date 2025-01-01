---
tags:
  - font
  - appearance
---
# `dpi`

Override the detected DPI (dots per inch) for the display.

This can be useful if the detected DPI is inaccurate and the text appears
either blurry or too small (especially if you are using a 4K display on X11 or
Wayland).

The default value is system specific:

|OS             |Standard Density        |High Density|
|---------------|------------------------|------------|
|macOS          |72.0                    |144.0       |
|Windows        |Probed from the display |Probed from the display |
|X11            |96.0                    |96.0                    |
|X11 (*version 20210314-114017-04b7cedd and later*)|Probed from `Xft.dpi`, fallback to 96.0 |Probed from `Xft.dpi`, fallback to 96.0 |
|X11 (*version 20210814-124438-54e29167 and later*)|Reads `Xft/DPI` via xsettings, fallback to `Xft.dpi`, then fallback to 96.0 | same as standard density |
|Wayland        |96.0                    |192.0       |

In macOS and Wayland environments there isn't strictly a system DPI value that
can be queried; instead standard density has a fixed value and the system will
inform WezTerm when the display is high density by communicating a scaling
factor for the display.

The Wayland protocol only allows for integer scaling factors, but some
compositors support fractional scaling.  That fractional scaling can result in
blurry text and you may wish to specify a DPI value to compensate.

On macOS the scaling factor changes based on the monitor on which the window is
displayed; dragging the window from a retina laptop display to an external
standard DPI display causes the window to automatically adjust to the DPI
scaling.

Microsoft Windows reports the true DPI for the monitor on which the window is
displayed, and will similarly adjust as the window is dragged between monitors.

DPI is poorly supported by X11 itself; while it is possible to query the
displays to determine their dimensions, the results are generally inaccurate.
It is common for X11 environments to publish an `Xft.dpi` value as a property
of the root window as a hint for the DPI of the display.  While that is a
reasonable workaround for a single-monitor system, it isn't ideal for a
multi-monitor setup where the monitors have varying DPIs.

