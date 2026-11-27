#!/usr/bin/env python3
"""Stand-in for a Linux file manager in the GUI end-to-end tests.

Owns org.freedesktop.FileManager1 on the session bus, the interface desktop
apps call to reveal an item, and appends every ShowItems/ShowFolders request
to the log file given as the first argument. CI runners have no desktop file
manager, so this records what OFLH asked the file manager to show.
"""
import sys

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

LOG = sys.argv[1]


class FileManager(dbus.service.Object):
    def _record(self, method, uris):
        with open(LOG, "a", encoding="utf-8") as log:
            for uri in uris:
                log.write(f"{method} {uri}\n")

    @dbus.service.method("org.freedesktop.FileManager1", in_signature="ass", out_signature="")
    def ShowItems(self, uris, startup_id):
        self._record("ShowItems", uris)

    @dbus.service.method("org.freedesktop.FileManager1", in_signature="ass", out_signature="")
    def ShowFolders(self, uris, startup_id):
        self._record("ShowFolders", uris)


DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()
name = dbus.service.BusName("org.freedesktop.FileManager1", bus)
FileManager(bus, "/org/freedesktop/FileManager1")
print("ready", flush=True)
GLib.MainLoop().run()
