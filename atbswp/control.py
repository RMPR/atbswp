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
import platform
import shutil
import subprocess
import tempfile
import time
from datetime import date

from pynput import keyboard
from pynput import mouse

import wx
import wx.adv

TMP_PATH = os.path.join(tempfile.gettempdir(), "tmp_atbswp")


class FileChooserCtrl:
    """
    Control class for both the open capture and save capture options

    keyboard.Keyword arguments
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
                shutil.copy(TMP_PATH, pathname)
            except IOError:
                wx.LogError(f"Cannot save current data in file {pathname}.")


class RecordCtrl:
    """
    Control class for the record button

    keyboard.Keyword arguments:
    capture -- current recording
    """
    def __init__(self):
        tmp_date = date.today().strftime("%Y %b %a")
        self.header = (
            f"# -*- coding: latin-1 -*- \n"
            f"# Created by atbswp (https://github.com/rmpr/atbswp)\n"
            f"# on {tmp_date}\n"
            f"import pyautogui \n"
            f"import time \n"
        )

        self.capture = [self.header]

    def on_move(self, x, y):
        if not self.recording:
            return False
        b = time.perf_counter()
        if int(b-self.last_time) > 0:
            self.capture.append(f"time.sleep ({b - self.last_time})")
        self.last_time = b

        self.capture.append(f"pyautogui.moveTo({x}, {y}, duration=0.1, _pause=False)")

    def on_click(self, x, y, button, pressed):
        if not self.recording:
            return False
        if pressed:
            if button == mouse.Button.left:
                self.capture.append(f"pyautogui.mouseDown ({x}, {y}, 'left')")
            elif button == mouse.Button.right:
                self.capture.append(f"pyautogui.mouseDown ({x}, {y}, 'right')")
            elif button == mouse.Button.middle:
                self.capture.append(f"pyautogui.mouseDown ({x}, {y}, 'middle')")
            else:
                wx.LogError("Mouse Button not recognized")
        else:
            if button == mouse.Button.left:
                self.capture.append(f"pyautogui.mouseUp ({x}, {y}, 'left')")
            elif button == mouse.Button.right:
                self.capture.append(f"pyautogui.mouseUp ({x}, {y}, 'right')")
            elif button == mouse.Button.middle:
                self.capture.append(f"pyautogui.mouseUp ({x}, {y}, 'middle')")
            else:
                wx.LogError("Mouse Button not recognized")

    def on_scroll(self, x, y, dx, dy):
        if not self.recording:
            return False
        print('Scrolled {0} at {1}'.format('down' if dy < 0 else 'up', ({x}, {y})))
        self.capture.append(f"pyautogui.scroll({dy})")

    def on_press(self, key):
        if not self.recording:
            return False
        b = time.perf_counter()
        self.capture.append(f"time.sleep ({b - self.last_time})")
        self.last_time = b

        try:
            self.capture.append(f"pyautogui.keyDown({repr(key.char)})")

        except AttributeError:
            if key == keyboard.Key.alt:
                if platform.system() == "Darwin":
                    self.capture.append(f"pyautogui.keyDown('option')")
                else:
                    self.capture.append(f"pyautogui.keyDown('alt')")
            elif key == keyboard.Key.alt_l:
                if platform.system() == "Darwin":
                    self.capture.append(f"pyautogui.keyDown('optionleft')")
                else:
                    self.capture.append(f"pyautogui.keyDown('altleft')")
            elif key == keyboard.Key.alt_r:
                if platform.system() == "Darwin":
                    self.capture.append(f"pyautogui.keyDown('optionright')")
                else:
                    self.capture.append(f"pyautogui.keyDown('altright')")
            elif key == keyboard.Key.alt_gr:
                self.capture.append(f"pyautogui.keyDown('altright')")
            elif key == keyboard.Key.backspace:
                self.capture.append(f"pyautogui.keyDown('backspace')")
            elif key == keyboard.Key.caps_lock:
                self.capture.append(f"pyautogui.keyDown('capslock')")
            elif key == keyboard.Key.cmd:
                if platform.system() == "Darwin":
                    self.capture.append(f"pyautogui.keyDown('command')")
                else:
                    self.capture.append(f"pyautogui.keyDown('winleft')")
            elif key == keyboard.Key.cmd_r:
                if platform.system() == "Darwin":
                    self.capture.append(f"pyautogui.keyDown('cmdright')")
                else:
                    self.capture.append(f"pyautogui.keyDown('winright')")
            elif key == keyboard.Key.ctrl:
                self.capture.append(f"pyautogui.keyDown('ctrlleft')")
            elif key == keyboard.Key.ctrl_r:
                self.capture.append(f"pyautogui.keyDown('ctrlright')")
            elif key == keyboard.Key.delete:
                self.capture.append(f"pyautogui.keyDown('delete')")
            elif key == keyboard.Key.down:
                self.capture.append(f"pyautogui.keyDown('down')")
            elif key == keyboard.Key.end:
                self.capture.append(f"pyautogui.keyDown('end')")
            elif key == keyboard.Key.enter:
                self.capture.append(f"pyautogui.keyDown('enter')")
            elif key == keyboard.Key.esc:
                self.capture.append(f"pyautogui.keyDown('esc')")
            elif key == keyboard.Key.f1:
                self.capture.append(f"pyautogui.keyDown('f1')")
            elif key == keyboard.Key.f2:
                self.capture.append(f"pyautogui.keyDown('f2')")
            elif key == keyboard.Key.f3:
                self.capture.append(f"pyautogui.keyDown('f3')")
            elif key == keyboard.Key.f4:
                self.capture.append(f"pyautogui.keyDown('f4')")
            elif key == keyboard.Key.f5:
                self.capture.append(f"pyautogui.keyDown('f5')")
            elif key == keyboard.Key.f6:
                self.capture.append(f"pyautogui.keyDown('f6')")
            elif key == keyboard.Key.f7:
                self.capture.append(f"pyautogui.keyDown('f7')")
            elif key == keyboard.Key.f8:
                self.capture.append(f"pyautogui.keyDown('f8')")
            elif key == keyboard.Key.f9:
                self.capture.append(f"pyautogui.keyDown('f9')")
            elif key == keyboard.Key.f10:
                self.capture.append(f"pyautogui.keyDown('f10')")
            elif key == keyboard.Key.f11:
                self.capture.append(f"pyautogui.keyDown('f11')")
            elif key == keyboard.Key.f12:
                self.capture.append(f"p2autogui.keyDown('f12')")
            elif key == keyboard.Key.home:
                self.capture.append(f"pyautogui.keyDown('home')")
            elif key == keyboard.Key.left:
                self.capture.append(f"pyautogui.keyDown('left')")
            elif key == keyboard.Key.page_down:
                self.capture.append(f"pyautogui.keyDown('pagedown')")
            elif key == keyboard.Key.page_up:
                self.capture.append(f"pyautogui.keyDown('pageup')")
            elif key == keyboard.Key.right:
                self.capture.append(f"pyautogui.keyDown('right')")
            elif key == keyboard.Key.shift:
                self.capture.append(f"pyautogui.keyDown('shift_left')")
            elif key == keyboard.Key.shift_r:
                self.capture.append(f"pyautogui.keyDown('shiftright')")
            elif key == keyboard.Key.space:
                self.capture.append(f"pyautogui.keyDown('space')")
            elif key == keyboard.Key.tab:
                self.capture.append(f"pyautogui.keyDown('tab')")
            elif key == keyboard.Key.up:
                self.capture.append(f"pyautogui.keyDown('up')")
            elif key == keyboard.Key.media_play_pause:
                self.capture.append(f"pyautogui.keyDown('playpause')")
            elif key == keyboard.Key.insert:
                self.capture.append(f"pyautogui.keyDown('insert')")
            elif key == keyboard.Key.menu:
                self.capture.append(f"### The menu key is not handled yet")
            elif key == keyboard.Key.num_lock:
                self.capture.append(f"pyautogui.keyDown('num_lock')")
            elif key == keyboard.Key.pause:
                self.capture.append(f"pyautogui.keyDown('pause')")
            elif key == keyboard.Key.print_screen:
                self.capture.append(f"pyautogui.keyDown('print_screen')")
            elif key == keyboard.Key.scroll_lock:
                self.capture.append(f"pyautogui.keyDown('scroll_lock')")
            else:
                self.capture.append(f"### {key} is not supported yet")

    def on_release(self, key):
        if not self.recording:
            return False
        #self.count_perf()
        if key == keyboard.Key.alt:
            if platform.system() == "Darwin":
                self.capture.append(f"pyautogui.keyUp('option')")
            else:
                self.capture.append(f"pyautogui.keyUp('alt')")
        elif key == keyboard.Key.alt_l:
            if platform.system() == "Darwin":
                self.capture.append(f"pyautogui.keyUp('optionleft')")
            else:
                self.capture.append(f"pyautogui.keyUp('altleft')")
        elif key == keyboard.Key.alt_r:
            if platform.system() == "Darwin":
                self.capture.append(f"pyautogui.keyUp('optionright')")
            else:
                self.capture.append(f"pyautogui.keyUp('altright')")
        elif key == keyboard.Key.alt_gr:
            self.capture.append(f"pyautogui.keyUp('altright')")
        elif key == keyboard.Key.backspace:
            self.capture.append(f"pyautogui.keyUp('backspace')")
        elif key == keyboard.Key.caps_lock:
            self.capture.append(f"pyautogui.keyUp('capslock')")
        elif key == keyboard.Key.cmd:
            if platform.system() == "Darwin":
                self.capture.append(f"pyautogui.keyUp('command')")
            else:
                self.capture.append(f"pyautogui.keyUp('winleft')")
        elif key == keyboard.Key.cmd_r:
            if platform.system() == "Darwin":
                self.capture.append(f"pyautogui.keyUp('cmdright')")
            else:
                self.capture.append(f"pyautogui.keyUp('winright')")
        elif key == keyboard.Key.ctrl:
            self.capture.append(f"pyautogui.keyUp('ctrlleft')")
        elif key == keyboard.Key.ctrl_r:
            self.capture.append(f"pyautogui.keyUp('ctrlright')")
        elif key == keyboard.Key.delete:
            self.capture.append(f"pyautogui.keyUp('delete')")
        elif key == keyboard.Key.down:
            self.capture.append(f"pyautogui.keyUp('down')")
        elif key == keyboard.Key.end:
            self.capture.append(f"pyautogui.keyUp('end')")
        elif key == keyboard.Key.enter:
            self.capture.append(f"pyautogui.keyUp('enter')")
        elif key == keyboard.Key.esc:
            self.capture.append(f"pyautogui.keyUp('esc')")
        elif key == keyboard.Key.f1:
            self.capture.append(f"pyautogui.keyUp('f1')")
        elif key == keyboard.Key.f2:
            self.capture.append(f"pyautogui.keyUp('f2')")
        elif key == keyboard.Key.f3:
            self.capture.append(f"pyautogui.keyUp('f3')")
        elif key == keyboard.Key.f4:
            self.capture.append(f"pyautogui.keyUp('f4')")
        elif key == keyboard.Key.f5:
            self.capture.append(f"pyautogui.keyUp('f5')")
        elif key == keyboard.Key.f6:
            self.capture.append(f"pyautogui.keyUp('f6')")
        elif key == keyboard.Key.f7:
            self.capture.append(f"pyautogui.keyUp('f7')")
        elif key == keyboard.Key.f8:
            self.capture.append(f"pyautogui.keyUp('f8')")
        elif key == keyboard.Key.f9:
            self.capture.append(f"pyautogui.keyUp('f9')")
        elif key == keyboard.Key.f10:
            self.capture.append(f"pyautogui.keyUp('f10')")
        elif key == keyboard.Key.f11:
            self.capture.append(f"pyautogui.keyUp('f11')")
        elif key == keyboard.Key.f12:
            self.capture.append(f"p2autogui.keyUp('f12')")
        elif key == keyboard.Key.home:
            self.capture.append(f"pyautogui.keyUp('home')")
        elif key == keyboard.Key.left:
            self.capture.append(f"pyautogui.keyUp('left')")
        elif key == keyboard.Key.page_down:
            self.capture.append(f"pyautogui.keyUp('pagedown')")
        elif key == keyboard.Key.page_up:
            self.capture.append(f"pyautogui.keyUp('pageup')")
        elif key == keyboard.Key.right:
            self.capture.append(f"pyautogui.keyUp('right')")
        elif key == keyboard.Key.shift:
            self.capture.append(f"pyautogui.keyUp('shift_left')")
        elif key == keyboard.Key.shift_r:
            self.capture.append(f"pyautogui.keyUp('shiftright')")
        elif key == keyboard.Key.space:
            self.capture.append(f"pyautogui.keyUp('space')")
        elif key == keyboard.Key.tab:
            self.capture.append(f"pyautogui.keyUp('tab')")
        elif key == keyboard.Key.up:
            self.capture.append(f"pyautogui.keyUp('up')")
        elif key == keyboard.Key.media_play_pause:
            self.capture.append(f"pyautogui.keyUp('playpause')")
        elif key == keyboard.Key.insert:
            self.capture.append(f"pyautogui.keyUp('insert')")
        elif key == keyboard.Key.menu:
            self.capture.append(f"### The menu key is not handled yet")
        elif key == keyboard.Key.num_lock:
            self.capture.append(f"pyautogui.keyUp('num_lock')")
        elif key == keyboard.Key.pause:
            self.capture.append(f"pyautogui.keyUp('pause')")
        elif key == keyboard.Key.print_screen:
            self.capture.append(f"pyautogui.keyUp('print_screen')")
        elif key == keyboard.Key.scroll_lock:
            self.capture.append(f"pyautogui.keyUp('scroll_lock')")
        else:
            self.capture.append(f"pyautogui.keyUp({repr(key)})")

    def action(self, event):
        listener_mouse = mouse.Listener(
            on_move=self.on_move,
            on_click=self.on_click,
            on_scroll=self.on_scroll)
        listener_keyboard = keyboard.Listener(
            on_press=self.on_press,
            on_release=self.on_release)

        if event.GetEventObject().GetValue():
            listener_keyboard.start()
            listener_mouse.start()
            self.last_time = time.perf_counter()
            self.recording = True
        else:
            self.recording = False
            with open(TMP_PATH, 'w') as f:
                f.seek(0)
                f.write("\n".join(self.capture))
                f.truncate()
            self.capture = [self.header]


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
        with open(TMP_PATH, 'r') as f:
            capture = f.read()
        exec(capture)
        event.GetEventObject().SetValue(False)


class SettingsCtrl:
    """
    Control class for the settings
    """
    def action(self, event):
        pass


class HelpCtrl:
    """
    Control class for the About menu
    """
    def action(self, event):
        info = wx.adv.AboutDialogInfo()
        info.Name = "atbswp"
        info.Version = "v0.1"
        info.Copyright = ("(C) 2019 Mairo Paul Rufus <akoudanilo@gmail.com>\n")
        info.Description = "Record mouse and keyboard actions and reproduce them identically at will"
        info.WebSite = ("https://github.com/atbswp", "Project homepage")
        info.Developers = ["Mairo Paul Rufus"]
        info.License = "GNU General Public License V3"
        wx.adv.AboutBox(info)
