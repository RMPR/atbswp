#!/usr/bin/env python3
# Record mouse and keyboard actions and reproduce them identically at will
#
# Copyright (C) 2019 Paul Mairo <github@rmpr.xyz>
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <http://www.gnu.org/licenses/>.

"""Entry point of the program."""

import gui


import wx


class Atbswp(wx.App):
    """Main class of the program."""

    def OnInit(self):
        """Initialize the main Window."""
        self.main = gui.MainDialog(None, wx.ID_ANY, "atbswp")
        self.SetTopWindow(self.main)
        self.main.Show()
        return True


if __name__ == "__main__":
    app = Atbswp(0)
    app.MainLoop()
