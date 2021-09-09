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

"""
This module contains all the classes needed to
create the GUI and handle non functionnal event
"""

import os
import sys
from pathlib import Path

import control

import settings

import wx
import wx.adv


class MainDialog(wx.Dialog, wx.MiniFrame):
    """Main Window, a dialog to display the app correctly even on tiling WMs."""

    app_text = ["Load Capture", "Save", "Start/Stop Capture", "Play", "Compile to executable",
                "Preferences", "Help"]
    settings_text = ["Play &Speed: Fast", "&Infinite Playback", "Set &Repeat Count", "Recording &Hotkey",
                     "&Playback Hotkey", "Always on &Top", "&Language", "&About", "&Exit"]

    def on_settings_click(self, event):
        """Triggered when the popup menu is clicked."""
        self.settings_popup()
        event.GetEventObject().PopupMenu(self.settings_popup())
        event.EventObject.Parent.panel.SetFocus()
        event.Skip()

    def settings_popup(self):
        """Build the popup menu."""
        menu = wx.Menu()
        # Replay fast
        ps = menu.Append(wx.ID_ANY, self.settings_text[0])
        self.Bind(wx.EVT_MENU,
                  control.SettingsCtrl.playback_speed,
                  ps)
        ps.Enable(False)

        #  Infinite Playback
        cp = menu.AppendCheckItem(wx.ID_ANY, self.settings_text[1])
        status = settings.CONFIG.getboolean('DEFAULT', 'Infinite Playback')
        cp.Check(status)
        self.Bind(wx.EVT_MENU,
                  control.SettingsCtrl.infinite_playback,
                  cp)

        # Repeat count
        self.Bind(wx.EVT_MENU, self.sc.repeat_count,
                  menu.Append(wx.ID_ANY, self.settings_text[2]))
        menu.AppendSeparator()

        # Recording hotkey
        self.Bind(wx.EVT_MENU,
                  control.SettingsCtrl.recording_hotkey,
                  menu.Append(wx.ID_ANY, self.settings_text[3]))

        # Playback hotkey
        self.Bind(wx.EVT_MENU,
                  control.SettingsCtrl.playback_hotkey,
                  menu.Append(wx.ID_ANY, self.settings_text[4]))
        menu.AppendSeparator()

        # Always on top
        aot = menu.AppendCheckItem(wx.ID_ANY, self.settings_text[5])
        status = settings.CONFIG.getboolean('DEFAULT', 'Always On Top')
        aot.Check(status)
        self.Bind(wx.EVT_MENU,
                  self.sc.always_on_top,
                  aot)

        # Language
        submenu = wx.Menu()
        # Workaround for users of the previous version
        current_lang = "en"
        try:
            current_lang = settings.CONFIG.get('DEFAULT', 'Language')
        except:
            pass

        for language in os.listdir(os.path.join(self.path, "lang")):
            lang_item = submenu.AppendRadioItem(wx.ID_ANY, language)
            self.Bind(wx.EVT_MENU,
                      self.sc.language,
                      lang_item)
            if language == current_lang:
                lang_item.Check(True)
        menu.AppendSubMenu(submenu, self.settings_text[6])

        # About
        self.Bind(wx.EVT_MENU,
                  self.on_about,
                  menu.Append(wx.ID_ABOUT, self.settings_text[7]))

        # Recording Timer
        self.Bind(wx.EVT_MENU,
                  control.RecordCtrl.recording_timer,
                  menu.Append(wx.ID_ANY, self.settings_text[8]))

        # Mouse speed
        self.Bind(wx.EVT_MENU,
                  control.RecordCtrl.mouse_speed,
                  menu.Append(wx.ID_ANY, self.settings_text[9]))
        return menu

    def __init__(self, *args, **kwds):
        """Build the interface."""
        if getattr(sys, 'frozen', False):
            self.path = sys._MEIPASS
        else:
            self.path = Path(__file__).parent.absolute()
        on_top = wx.DEFAULT_DIALOG_STYLE
        on_top = on_top if not settings.CONFIG.getboolean('DEFAULT', 'Always On Top') \
            else on_top | wx.STAY_ON_TOP
        kwds["style"] = kwds.get("style", 0) | on_top
        wx.Dialog.__init__(self, *args, **kwds)
        self.panel = wx.Panel(self)
        self.icon = wx.Icon(os.path.join(self.path, "img", "icon.png"))
        self.SetIcon(self.icon)
        self.taskbar = TaskBarIcon(self)
        self.taskbar.SetIcon(self.icon, "atbswp")

        locale = self.__load_locale()
        self.app_text, self.settings_text = locale[:7], locale[7:]
        self.file_open_button = wx.BitmapButton(self,
                                                wx.ID_ANY,
                                                wx.Bitmap(os.path.join(self.path, "img", "file-upload.png"),
                                                          wx.BITMAP_TYPE_ANY))
        self.file_open_button.SetToolTip(self.app_text[0])
        self.save_button = wx.BitmapButton(self,
                                           wx.ID_ANY,
                                           wx.Bitmap(os.path.join(self.path, "img", "save.png"),
                                                     wx.BITMAP_TYPE_ANY))
        self.save_button.SetToolTip(self.app_text[1])
        self.record_button = wx.BitmapToggleButton(self,
                                                   wx.ID_ANY,
                                                   wx.Bitmap(os.path.join(self.path, "img", "video.png"),
                                                             wx.BITMAP_TYPE_ANY))
        self.record_button.SetToolTip(self.app_text[2])
        self.play_button = wx.BitmapToggleButton(self,
                                                 wx.ID_ANY,
                                                 wx.Bitmap(os.path.join(self.path, "img", "play-circle.png"),
                                                           wx.BITMAP_TYPE_ANY))
        self.remaining_plays = wx.StaticText(self, label=settings.CONFIG.get("DEFAULT", "Repeat Count"),
                                             style=wx.ALIGN_CENTRE_HORIZONTAL)
        self.play_button.SetToolTip(self.app_text[3])
        self.compile_button = wx.BitmapButton(self,
                                              wx.ID_ANY,
                                              wx.Bitmap(os.path.join(self.path, "img", "download.png"),
                                                        wx.BITMAP_TYPE_ANY))
        self.compile_button.SetToolTip(self.app_text[4])
        self.settings_button = wx.BitmapButton(self,
                                               wx.ID_ANY,
                                               wx.Bitmap(os.path.join(self.path, "img", "cog.png"),
                                                         wx.BITMAP_TYPE_ANY))
        self.settings_button.SetToolTip(self.app_text[5])

        self.help_button = wx.BitmapButton(self,
                                           wx.ID_ANY,
                                           wx.Bitmap(os.path.join(self.path, "img", "question-circle.png"),
                                                     wx.BITMAP_TYPE_ANY))
        self.help_button.SetToolTip(self.app_text[6])

        self.__add_bindings()
        self.__set_properties()
        self.__do_layout()

    def __load_locale(self):
        """Load the interface in user-defined language (default english)."""
        try:
            lang = settings.CONFIG.get('DEFAULT', 'Language')
            locale = open(os.path.join(self.path, "lang", lang)
                          ).read().splitlines()
        except:
            return self.app_text + self.settings_text

        return locale

    def __add_bindings(self):
        # file_save_ctrl
        self.fsc = control.FileChooserCtrl(self)
        self.Bind(wx.EVT_BUTTON, self.fsc.load_file, self.file_open_button)
        self.Bind(wx.EVT_BUTTON, self.fsc.save_file, self.save_button)

        # record_button_ctrl
        self.rbc = control.RecordCtrl()
        self.Bind(wx.EVT_TOGGLEBUTTON, self.rbc.action, self.record_button)

        # play_button_ctrl
        self.pbc = control.PlayCtrl()
        self.Bind(wx.EVT_TOGGLEBUTTON, self.pbc.action, self.play_button)

        # Handle the event returned after a playback has completed
        self.Bind(self.pbc.EVT_THREAD_END, self.on_thread_end)

        # compile_button_ctrl
        self.Bind(wx.EVT_BUTTON, control.CompileCtrl.compile,
                  self.compile_button)

        # help_button_ctrl
        self.Bind(wx.EVT_BUTTON, control.HelpCtrl.action, self.help_button)

        # settings_button_ctrl
        self.Bind(wx.EVT_BUTTON, self.on_settings_click, self.settings_button)
        self.sc = control.SettingsCtrl(self)

        self.Bind(wx.EVT_CLOSE, self.on_close_dialog)

        # Handle keyboard shortcuts
        self.panel.Bind(wx.EVT_KEY_UP, self.on_key_press)

        self.panel.SetFocus()

    def __set_properties(self):
        self.file_open_button.SetSize(self.file_open_button.GetBestSize())
        self.save_button.SetSize(self.save_button.GetBestSize())
        self.record_button.SetSize(self.record_button.GetBestSize())
        self.play_button.SetSize(self.play_button.GetBestSize())
        self.compile_button.SetSize(self.compile_button.GetBestSize())
        self.settings_button.SetSize(self.settings_button.GetBestSize())
        self.help_button.SetSize(self.help_button.GetBestSize())

    def __do_layout(self):
        self.remaining_plays.Position = (256, 0)
        self.remaining_plays.SetBackgroundColour((0, 0, 0))
        self.remaining_plays.SetForegroundColour((255, 255, 255))
        main_sizer = wx.BoxSizer(wx.HORIZONTAL)
        main_sizer.Add(self.panel)
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
        """ Create manually the event when the correct key is pressed."""
        keycode = event.GetKeyCode()

        if keycode == wx.WXK_F1:
            control.HelpCtrl.action(wx.PyCommandEvent(wx.wxEVT_BUTTON))

        elif keycode == settings.CONFIG.getint('DEFAULT', 'Recording Hotkey'):
            btn_event = wx.CommandEvent(wx.wxEVT_TOGGLEBUTTON)
            btn_event.EventObject = self.record_button
            if not self.record_button.Value:
                self.record_button.Value = True
                self.rbc.action(btn_event)
            else:
                self.record_button.Value = False
                self.rbc.action(btn_event)

        elif keycode == settings.CONFIG.getint('DEFAULT', 'Playback Hotkey'):
            if not self.play_button.Value:
                self.play_button.Value = True
                btn_event = wx.CommandEvent(wx.wxEVT_TOGGLEBUTTON)
                btn_event.EventObject = self.play_button
                self.pbc.action(btn_event)
            else:
                self.play_button.Value = False

        elif keycode == ord("R") and event.CmdDown():
            menu_event = wx.CommandEvent(wx.wxEVT_MENU)
            self.sc.repeat_count(menu_event)

        elif keycode == ord("O") and event.CmdDown():
            btn_event = wx.CommandEvent(wx.wxEVT_TOGGLEBUTTON)
            btn_event.EventObject = self.file_open_button
            self.fsc.load_file(btn_event)

        elif keycode == ord("S") and event.CmdDown():
            btn_event = wx.CommandEvent(wx.wxEVT_TOGGLEBUTTON)
            btn_event.EventObject = self.save_button
            self.fsc.save_file(btn_event)

        event.Skip()

    def on_thread_end(self, event):
        self.play_button.Value = event.toggle_value
        self.remaining_plays.Label = str(event.count) if event.count > 0 else \
            str(settings.CONFIG.getint('DEFAULT', 'Repeat Count'))
        self.remaining_plays.Update()

    def on_exit_app(self, event):
        """Clean exit saving the settings."""
        settings.save_config()
        self.Destroy()
        self.taskbar.Destroy()

    def on_close_dialog(self, event):
        """Confirm exit."""
        dialog = wx.MessageDialog(self,
                                  message="Are you sure you want to quit?",
                                  caption="Confirm Exit",
                                  style=wx.YES_NO,
                                  pos=wx.DefaultPosition)
        response = dialog.ShowModal()

        if (response == wx.ID_YES):
            self.on_exit_app(event)
        else:
            event.StopPropagation()

    def on_about(self, event):
        """About dialog."""
        info = wx.adv.AboutDialogInfo()
        info.Name = "atbswp"
        info.Version = f"{settings.VERSION}"
        info.Copyright = (f"©{settings.YEAR} Paul Mairo <github@rmpr.xyz>\n")
        info.Description = "Record mouse and keyboard actions and reproduce them identically at will"
        info.WebSite = ("https://github.com/atbswp", "Project homepage")
        info.Developers = ["Paul Mairo"]
        info.License = "GNU General Public License V3"
        info.Icon = self.icon
        wx.adv.AboutBox(info)


class TaskBarIcon(wx.adv.TaskBarIcon):
    """Taskbar showing the state of the recording."""

    def __init__(self, parent):
        self.parent = parent
        super(TaskBarIcon, self).__init__()
