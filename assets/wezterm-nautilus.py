# Copyright (C) 2022 Sebastian Wiesner <sebastian@swsnr.de>
#
# This program is free software; you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation; either version 2 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License along
# with this program; if not, write to the Free Software Foundation, Inc.,
# 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.

import time
import os.path

from gi import require_version
require_version('Nautilus', '3.0')
from gi.repository import Nautilus, GObject, Gio, GLib


def sd_booted():
    """
    Check if this system runs under systemd, per sd_booted(3).
    """
    return os.path.isdir('/run/systemd/system/')


class SystemdManager():
    def __init__(self, proxy):
        self._proxy = proxy

    def move_to_dedicated_scope(self, pid):
        props = [("PIDs", GLib.Variant('au', [pid])),
            ('CollectMode', GLib.Variant('s', 'inactive-or-failed'))]
        name = f'app-nautilus-org.wezfurlong.wezterm-{pid}.scope'
        args = GLib.Variant('(ssa(sv)a(sa(sv)))', (name, 'fail', props, []))
        self._proxy.call_sync('StartTransientUnit', args,
                Gio.DBusCallFlags.NO_AUTO_START, 500, None)

    @classmethod
    def try_connect(cls, bus):
        if sd_booted():
            proxy = Gio.DBusProxy.new_sync(bus,
                    Gio.DBusProxyFlags.NONE,
                    None,
                    "org.freedesktop.systemd1",
                    "/org/freedesktop/systemd1",
                    "org.freedesktop.systemd1.Manager", None)
            return cls(proxy)
        else:
            return None


def _open_in_existing_terminal(path):
    cmd = ['wezterm', 'cli', 'spawn', '--cwd', path]
    child = Gio.Subprocess.new(cmd, Gio.SubprocessFlags.NONE)
    child.wait_check()

def _wezterm_running():
    cmd = ['wezterm', 'cli', 'list']
    flags = Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE
    child = Gio.Subprocess.new(cmd, flags)
    child.wait()
    return child.get_if_exited() and child.get_exit_status() == 0


class OpenInWezTermAction(GObject.GObject, Nautilus.MenuProvider):
    def __init__(self):
        super().__init__()
        self._systemd = SystemdManager.try_connect(Gio.bus_get_sync(Gio.BusType.SESSION, None))

    def _open_new_terminal(self, path):
        cmd = ['wezterm', 'start', '--cwd', path]
        child = Gio.Subprocess.new(cmd, Gio.SubprocessFlags.NONE)
        if self._systemd:
            # Move new terminal into a dedicated systemd scope to make systemd
            # track the terminal separately; in particular this makes systemd
            # keep a separate CPU and memory account for Wezterm which in turn
            # ensures that oomd doesn't take nautilus down if a process in
            # wezterm consumes a lot of memory.
            self._systemd.move_to_dedicated_scope(int(child.get_identifier()))

    def _open_paths_in_new_or_existing_terminal(self, paths):
        if _wezterm_running():
            for path in paths:
                _open_in_existing_terminal(path)
        else:
            self._open_new_terminal(paths[0])
            time.sleep(0.5)
            for path in paths[1:]:
                _open_in_existing_terminal(path)

    def _menu_item_activated(self, _menu, paths):
        self._open_paths_in_new_or_existing_terminal(paths)

    def _make_item(self, name, paths):
        item = Nautilus.MenuItem(name=name, label='Open in WezTerm',
            icon='org.wezfurlong.wezterm')
        item.connect('activate', self._menu_item_activated, paths)
        return item

    def _paths_to_open(self, files):
        paths = []
        for file in files:
            location = file.get_location() if file.is_directory() else file.get_parent_location()
            path = location.get_path()
            if path and path not in paths:
                paths.append(path)
        if 10 < len(paths):
            # Let's not open anything if the user selected a lot of directories,
            # to avoid accidentally spamming their desktop with dozends of
            # new windows or tabs.  Ten is a totally arbitrary limit :)
            return []
        else:
            return paths

    def get_file_items(self, window, files):
        paths = self._paths_to_open(files)
        if paths:
            return [self._make_item(name='WezTermNautilus::open_in_wezterm', paths=paths)]
        else:
            return []

    def get_background_items(self, window, file):
        paths = self._paths_to_open([file])
        if paths:
            return [self._make_item(name='WezTermNautilus::open_folder_in_wezterm', paths=paths)]
        else:
            return []
