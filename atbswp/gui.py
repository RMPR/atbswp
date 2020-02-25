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

"""
This module contains all the classes needed to
create the GUI and handle non functionnal event
"""

import os
from pathlib import Path

import control

import wx
import wx.adv


class MainDialog(wx.Dialog, wx.MiniFrame):
    """
    Main windows of the app, it's a dialog to display_button the app correctly
    even with tiling WMs
    """
    app_text = ["Load Capture", "Save", "Start/Stop Capture", "Play",
                "Compile to executable", "Preferences", "Help"]
    settings_text = ["&Continuous Playback", "&Repeat Playback Loops",
                     "&Recording Hotkey", "&Playback Hotkey", "&Always on Top",
                     "About"]

    def on_settings_click(self, event):
        self.MakeThePopup()
        self.settings_button.PopupMenu(self.settingspopupmenu)
        event.Skip()

    def MakeThePopup(self):
        menu = wx.Menu()
        menu.Append(wx.ID_ANY, self.settings_text[0])
        menu.Append(wx.ID_ANY, self.settings_text[1])
        menu.AppendSeparator()
        menu.Append(wx.ID_ANY, self.settings_text[2])
        menu.Append(wx.ID_ANY, self.settings_text[3])
        menu.AppendSeparator()
        menu.Append(wx.ID_ANY, self.settings_text[4])
        menu.AppendSeparator()
        menu.Append(wx.ID_ANY, self.settings_text[5])
        self.settingspopupmenu = menu

    def __init__(self, *args, **kwds):
        """
        Build the interface
        """
        path = Path(__file__).parent.absolute()
        kwds["style"] = kwds.get("style", 0) | wx.DEFAULT_DIALOG_STYLE
        wx.Dialog.__init__(self, *args, **kwds)
        self.SetIcon(wx.Icon(os.path.join(path, "img", "icon.png")))
        self.file_open_button = wx.BitmapButton(self,
                                                wx.ID_ANY,
                                                wx.Bitmap(os.path.join(path, "img", "file-upload.png"),
                                                          wx.BITMAP_TYPE_ANY))
        self.file_open_button.SetToolTip(self.app_text[0])
        self.save_button = wx.BitmapButton(self,
                                           wx.ID_ANY,
                                           wx.Bitmap(os.path.join(path, "img", "save.png"),
                                                     wx.BITMAP_TYPE_ANY))
        self.save_button.SetToolTip(self.app_text[1])
        self.record_button = wx.BitmapToggleButton(self,
                                                   wx.ID_ANY,
                                                   wx.Bitmap(os.path.join(path, "img", "video.png"),
                                                             wx.BITMAP_TYPE_ANY))
        self.record_button.SetToolTip(self.app_text[2])
        self.play_button = wx.BitmapToggleButton(self,
                                                 wx.ID_ANY,
                                                 wx.Bitmap(os.path.join(path, "img", "play-circle.png"),
                                                           wx.BITMAP_TYPE_ANY))
        self.play_button.SetToolTip(self.app_text[3])
        self.compile_button = wx.BitmapButton(self,
                                              wx.ID_ANY,
                                              wx.Bitmap(os.path.join(path, "img", "download.png"),
                                                        wx.BITMAP_TYPE_ANY))
        self.compile_button.SetToolTip(self.app_text[4])
        self.settings_button = wx.BitmapButton(self,
                                               wx.ID_ANY,
                                               wx.Bitmap(os.path.join(path, "img", "cog.png"),
                                                         wx.BITMAP_TYPE_ANY))
        self.settings_button.SetToolTip(self.app_text[5])

        self.help_button = wx.BitmapButton(self,
                                           wx.ID_ANY,
                                           wx.Bitmap(os.path.join(path, "img", "question-circle.png"),
                                                     wx.BITMAP_TYPE_ANY))
        self.help_button.SetToolTip(self.app_text[6])

        self.__add_bindings()
        self.__set_properties()
        self.__do_layout()

    def load_locale(self):
        """
        Load the interface in user-defined language (default english)
        """
        # TODO
        pass

    def __add_bindings(self):
        # file_save_ctrl
        self.fsc = control.FileChooserCtrl(self)
        self.Bind(wx.EVT_BUTTON, self.fsc.load_file, self.file_open_button)
        self.Bind(wx.EVT_BUTTON, self.fsc.save_file, self.save_button)

        # record_button_ctrl
        rbc = control.RecordCtrl()
        self.Bind(wx.EVT_TOGGLEBUTTON, rbc.action, self.record_button)

        # play_button_ctrl
        pbc = control.PlayCtrl()
        self.Bind(wx.EVT_TOGGLEBUTTON, pbc.action, self.play_button)

        # help_button_ctrl
        hbc = control.HelpCtrl()
        self.Bind(wx.EVT_BUTTON, hbc.action, self.help_button)

        # settings_button_ctrl
        sbc = control.SettingsCtrl()
        self.Bind(wx.EVT_BUTTON, sbc.action, self.settings_button)
        self.Bind(wx.EVT_BUTTON, self.on_settings_click, self.settings_button)

        self.Bind(wx.EVT_CLOSE, self.OnCloseFrame)

    def _toggle_after_execution(self, message=""):
        btnEvent = wx.CommandEvent(wx.wxEVT_BUTTON)
        btnEvent.EventObject = self.file_open_button
        btnEvent.Checked = True
        print(self.ProcessEvent(btnEvent))
        self.play_button.Update()

    def __set_properties(self):
        self.file_open_button.SetSize(self.file_open_button.GetBestSize())
        self.save_button.SetSize(self.save_button.GetBestSize())
        self.record_button.SetSize(self.record_button.GetBestSize())
        self.play_button.SetSize(self.play_button.GetBestSize())
        self.compile_button.SetSize(self.compile_button.GetBestSize())
        self.settings_button.SetSize(self.settings_button.GetBestSize())
        self.help_button.SetSize(self.help_button.GetBestSize())

    def __do_layout(self):
        main_sizer = wx.BoxSizer(wx.HORIZONTAL)
        main_sizer.Add(self.file_open_button, 0, 0, 0)
        main_sizer.Add(self.save_button, 0, 0, 0)
        main_sizer.Add(self.record_button, 0, 0, 0)
        main_sizer.Add(self.play_button, 0, 0, 0)
        main_sizer.Add(self.compile_button, 0, 0, 0)
        main_sizer.Add(self.settings_button, 0, 0, 0)
        main_sizer.Add(self.help_button, 0, 0, 0)
        self.SetSizer(main_sizer)
        self.Centre()
        main_sizer.Fit(self)
        self.Layout()

    def OnExitApp(self, event):
        self.Destroy()

    def OnCloseFrame(self, event):
        dialog = wx.MessageDialog(self,
                                  message="Are you sure you want to quit?",
                                  caption="Caption",
                                  style=wx.YES_NO,
                                  pos=wx.DefaultPosition)
        response = dialog.ShowModal()

        if (response == wx.ID_YES):
            self.OnExitApp(event)
        else:
            event.StopPropagation()


TRAY_TOOLTIP = 'Name'
TRAY_ICON = 'icon.png'

def create_menu_item(menu, label, func):
    item = wx.MenuItem(menu, -1, label)
    menu.Bind(wx.EVT_MENU, func, id=item.GetId())
    menu.Append(item)
    return item


class TaskBarIcon(wx.adv.TaskBarIcon):
    def __init__(self, frame):
        self.frame = frame
        super(TaskBarIcon, self).__init__()
        self.set_icon(TRAY_ICON)
        self.Bind(wx.adv.EVT_TASKBAR_LEFT_DOWN, self.on_left_down)

    def CreatePopupMenu(self):
        menu = wx.Menu()
        create_menu_item(menu, 'Site', self.on_hello)
        menu.AppendSeparator()
        create_menu_item(menu, 'Exit', self.on_exit)
        return menu

    def set_icon(self, path):
        icon = wx.Icon(path)
        self.SetIcon(icon, TRAY_TOOLTIP)

    def on_left_down(self, event):
        print ('Tray icon was left-clicked.')

    def on_hello(self, event):
        print ('Hello, world!')

    def on_exit(self, event):
        wx.CallAfter(self.Destroy)
        self.frame.Close()

class App(wx.App):
    def OnInit(self):
        frame=wx.Frame(None)
        self.SetTopWindow(frame)
        TaskBarIcon(frame)
        return True

def main():
    app = App(False)
    app.MainLoop()


if __name__ == '__main__':
    main()
