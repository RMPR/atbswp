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


APP_TEXT = ["Load Capture", "Save", "Start/Stop Capture", "Play",
            "Compile to executable", "Preferences", "Help"]
SETTINGS_TEXT = ["Play &Speed: Fast", "&Continuous Playback", 
                 "&Repeat Playback Loops", "Recording &Hotkey", 
                 "&Playback Hotkey", "Always on &Top", "&About", 
                 "&Exit"]


class MainDialog(wx.Dialog, wx.MiniFrame):
    """
    Main windows of the app, it's a dialog to display_button the app correctly
    even with tiling WMs
    """
    def on_settings_click(self, event):
        self.settings_popup()
        event.GetEventObject().PopupMenu(self.settings_popup())
        event.Skip()

    def settings_popup(self):
        menu = wx.Menu()
        ps = menu.Append(wx.ID_ANY, SETTINGS_TEXT[0])
        self.Bind(wx.EVT_MENU,
                  control.SettingsCtrl.playback_speed,
                  ps)
        ps.Enable(False)
        self.Bind(wx.EVT_MENU,
                  control.SettingsCtrl.continuous_playback,
                  menu.Append(wx.ID_ANY, SETTINGS_TEXT[1]))
        self.Bind(wx.EVT_MENU,
                  control.SettingsCtrl.repeat_count,
                  menu.Append(wx.ID_ANY, SETTINGS_TEXT[2]))
        menu.AppendSeparator()
        self.Bind(wx.EVT_MENU,
                  control.SettingsCtrl.recording_hotkey,
                  menu.Append(wx.ID_ANY, SETTINGS_TEXT[3]))
        self.Bind(wx.EVT_MENU,
                  control.SettingsCtrl.playback_hotkey,
                  menu.Append(wx.ID_ANY, SETTINGS_TEXT[4]))
        menu.AppendSeparator()
        self.Bind(wx.EVT_MENU,
                  control.SettingsCtrl.always_on_top,
                  menu.Append(wx.ID_ANY, SETTINGS_TEXT[5]))
        self.Bind(wx.EVT_MENU,
                  self.on_about,
                  menu.Append(wx.ID_ANY, SETTINGS_TEXT[6]))
        self.Bind(wx.EVT_MENU,
                  self.on_close_dialog,
                  menu.Append(wx.ID_ANY, SETTINGS_TEXT[7]))
        return menu

    def __init__(self, *args, **kwds):
        """
        Build the interface
        """
        path = Path(__file__).parent.absolute()
        kwds["style"] = kwds.get("style", 0) | wx.DEFAULT_DIALOG_STYLE
        wx.Dialog.__init__(self, *args, **kwds)
        self.icon = wx.Icon(os.path.join(path, "img", "icon.png"))
        self.SetIcon(self.icon)
        self.taskbar = TaskBarIcon(self)
        self.taskbar.SetIcon(self.icon, "atbswp")

        self.file_open_button = wx.BitmapButton(self,
                                                wx.ID_ANY,
                                                wx.Bitmap(os.path.join(path, "img", "file-upload.png"),
                                                          wx.BITMAP_TYPE_ANY))
        self.file_open_button.SetToolTip(APP_TEXT[0])
        self.save_button = wx.BitmapButton(self,
                                           wx.ID_ANY,
                                           wx.Bitmap(os.path.join(path, "img", "save.png"),
                                                     wx.BITMAP_TYPE_ANY))
        self.save_button.SetToolTip(APP_TEXT[1])
        self.record_button = wx.BitmapToggleButton(self,
                                                   wx.ID_ANY,
                                                   wx.Bitmap(os.path.join(path, "img", "video.png"),
                                                             wx.BITMAP_TYPE_ANY))
        self.record_button.SetToolTip(APP_TEXT[2])
        self.play_button = wx.BitmapToggleButton(self,
                                                 wx.ID_ANY,
                                                 wx.Bitmap(os.path.join(path, "img", "play-circle.png"),
                                                           wx.BITMAP_TYPE_ANY))
        self.play_button.SetToolTip(APP_TEXT[3])
        self.compile_button = wx.BitmapButton(self,
                                              wx.ID_ANY,
                                              wx.Bitmap(os.path.join(path, "img", "download.png"),
                                                        wx.BITMAP_TYPE_ANY))
        self.compile_button.SetToolTip(APP_TEXT[4])
        self.settings_button = wx.BitmapButton(self,
                                               wx.ID_ANY,
                                               wx.Bitmap(os.path.join(path, "img", "cog.png"),
                                                         wx.BITMAP_TYPE_ANY))
        self.settings_button.SetToolTip(APP_TEXT[5])

        self.help_button = wx.BitmapButton(self,
                                           wx.ID_ANY,
                                           wx.Bitmap(os.path.join(path, "img", "question-circle.png"),
                                                     wx.BITMAP_TYPE_ANY))
        self.help_button.SetToolTip(APP_TEXT[6])

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
        self.Bind(wx.EVT_BUTTON, self.on_settings_click, self.settings_button)

        self.Bind(wx.EVT_CLOSE, self.on_close_dialog)

        self.Bind(wx.EVT_KEY_DOWN, self.on_key_press)

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

    def on_key_press(self, event):
        keycode = event.GetKeycode()

        if keycode == wx.WXK_F1:
            control.HelpCtrl.action()
        else:
            event.Skip()

    def on_exit_app(self, event):
        self.Destroy()
        self.taskbar.Destroy()

    def on_close_dialog(self, event):
        dialog = wx.MessageDialog(self,
                                  message="Are you sure you want to quit?",
                                  caption="Caption",
                                  style=wx.YES_NO,
                                  pos=wx.DefaultPosition)
        response = dialog.ShowModal()

        if (response == wx.ID_YES):
            self.on_exit_app(event)
        else:
            event.StopPropagation()

    def on_about(self, event):
        info = wx.adv.AboutDialogInfo()
        info.Name = "atbswp"
        info.Version = "v0.1"
        info.Copyright = ("(C) 2019 Mairo Paul Rufus <akoudanilo@gmail.com>\n")
        info.Description = "Record mouse and keyboard actions and reproduce them identically at will"
        info.WebSite = ("https://github.com/atbswp", "Project homepage")
        info.Developers = ["Mairo Paul Rufus"]
        info.License = "GNU General Public License V3"
        info.Icon = self.icon
        wx.adv.AboutBox(info)


class TaskBarIcon(wx.adv.TaskBarIcon):
    def __init__(self, parent):
        self.parent = parent
        super(TaskBarIcon, self).__init__()

