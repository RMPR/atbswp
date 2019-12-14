#!/usr/bin/env python3
# Record mouse and keyboard actions and reproduce them identically at will
#
# Copyright (C) 2019 Mairo Rufus <akoudanilo@gmail.com>
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

import os
import subprocess
import tempfile

import wx
import wx.adv

TMP_PATH = os.path.join(tempfile.gettempdir(), "tmp_atbswp")


class FileChooserCtrl:
    """
    Control class for both the open capture and save capture options

    Keyword arguments
    capture -- content of the temporary file
    """

    capture = str()

    def __init__(self, parent):
        self.parent = parent

    def load_content(self, path):
        # TODO: Better input control
        if not path or not os.path.isfile(path):
            return None
        with open(path, 'r') as f:
            return f.read()

    def load_file(self, event):
        global TMP_PATH
        title = "Choose a capture file:"
        dlg = wx.FileDialog(self.parent,
                            message=title,
                            defaultDir="~",
                            style=wx.DD_DEFAULT_STYLE)
        if dlg.ShowModal() == wx.ID_OK:
            self.capture = self.load_content(dlg.GetPath())
            with open(TMP_PATH, 'w') as f:
                f.write(self.capture)
        dlg.Destroy()

    def save_file(self, event):
        with wx.FileDialog(self.parent, "Save capture file", wildcard="*",
                           style=wx.FD_SAVE | wx.FD_OVERWRITE_PROMPT) as fileDialog:

            if fileDialog.ShowModal() == wx.ID_CANCEL:
                return     # the user changed their mind

            # save the current contents in the file
            pathname = fileDialog.GetPath()
            try:
                with open(pathname, 'w') as file:
                    file.write(self.capture)
            except IOError:
                wx.LogError(f"Cannot save current data in file {pathname}.")


class RecordCtrl:
    """
    Control class for the record button
    """
    def action(self, event):
        pass


class PlayCtrl:
    """
    Control class for the play button
    """
    global TMP_PATH

    def __init__(self):
        pass

    def action(self, event):
        if TMP_PATH is None or not os.path.isfile(TMP_PATH):
            wx.LogError(f"{TMP_PATH} doesn't seem to exist")
            return
        subprocess.Popen(["python", TMP_PATH])


class SettingsCtrl:
    """
    Control class for the settings
    """
    def action(self, event):
        print("we reach here hey")


class HelpCtrl:
    """
    Control class for the About menu
    """
    def action(self, event):
        info = wx.adv.AboutDialogInfo()
        info.Name = "About"
        info.Version = "atbswp v0.1"
        info.Copyright = ("(C) 2019 Mairo Paul Rufus <akoudanilo@gmail.com>\n")
        info.Description = "Record mouse and keyboard actions and reproduce them identically at will"
        info.WebSite = ("https://github.com/atbswp", "Project homepage")
        info.Developers = ["Mairo Paul Rufus"]
        info.License = "GNU General Public License V3"
        wx.adv.AboutBox(info)
