#!/usr/bin/env python3
"""A stand-in for xdg-desktop-portal's RemoteDesktop interface.

Speaks exactly enough D-Bus for the player's portal code path:
CreateSession / SelectDevices / Start answer through Request.Response
signals, ConnectToEIS hands back a socket connected to an EIS server
(tests/eis_sink).  Everything it sees is appended to LOG so the test can
assert on options such as restore_token.

usage: fake_portal.py EIS_SOCKET_PATH LOG
"""
import socket
import sys

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

BUS_NAME = "org.freedesktop.portal.Desktop"
OBJ_PATH = "/org/freedesktop/portal/desktop"
RD_IFACE = "org.freedesktop.portal.RemoteDesktop"
REQ_IFACE = "org.freedesktop.portal.Request"
SESSION_PATH = "/org/freedesktop/portal/desktop/session/fake/s1"
RESTORE_TOKEN = "fake-restore-token-42"


def log(msg):
    with open(sys.argv[2], "a") as f:
        f.write(msg + "\n")
    print(msg, flush=True)


class Request(dbus.service.Object):
    @dbus.service.signal(REQ_IFACE, signature="ua{sv}")
    def Response(self, response, results):
        pass

    @dbus.service.method(REQ_IFACE)
    def Close(self):
        pass


class RemoteDesktop(dbus.service.Object):
    def __init__(self, bus):
        super().__init__(bus, OBJ_PATH)
        self.bus = bus
        self.sockets = []

    def _respond(self, sender, options, results):
        token = str(options.get("handle_token", "t"))
        mangled = sender[1:].replace(".", "_")
        path = f"/org/freedesktop/portal/desktop/request/{mangled}/{token}"
        req = Request(self.bus, path)

        def fire():
            req.Response(dbus.UInt32(0), dbus.Dictionary(results, signature="sv"))
            return False

        GLib.idle_add(fire)
        return dbus.ObjectPath(path)

    @dbus.service.method(RD_IFACE, in_signature="a{sv}", out_signature="o", sender_keyword="sender")
    def CreateSession(self, options, sender=None):
        log(f"CreateSession token={options.get('handle_token')} session_token={options.get('session_handle_token')}")
        return self._respond(sender, options, {"session_handle": dbus.String(SESSION_PATH)})

    @dbus.service.method(RD_IFACE, in_signature="oa{sv}", out_signature="o", sender_keyword="sender")
    def SelectDevices(self, session, options, sender=None):
        log(f"SelectDevices session={session} types={int(options.get('types', 0))} "
            f"persist_mode={int(options.get('persist_mode', 0))} restore_token={options.get('restore_token', '')}")
        return self._respond(sender, options, {})

    @dbus.service.method(RD_IFACE, in_signature="osa{sv}", out_signature="o", sender_keyword="sender")
    def Start(self, session, parent, options, sender=None):
        log(f"Start session={session} parent='{parent}'")
        return self._respond(sender, options, {"restore_token": dbus.String(RESTORE_TOKEN),
                                               "devices": dbus.UInt32(3)})

    @dbus.service.method(RD_IFACE, in_signature="oa{sv}", out_signature="h")
    def ConnectToEIS(self, session, options):
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.connect(sys.argv[1])
        self.sockets.append(s)  # keep our end alive until exit
        log(f"ConnectToEIS session={session}")
        return dbus.types.UnixFd(s.fileno())

    @dbus.service.method(RD_IFACE, in_signature="", out_signature="u")
    def version(self):
        return dbus.UInt32(2)


def main():
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 64
    DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    name = dbus.service.BusName(BUS_NAME, bus)
    RemoteDesktop(bus)
    log("fake portal ready")
    GLib.MainLoop().run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
