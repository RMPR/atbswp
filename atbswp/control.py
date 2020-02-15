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
import tempfile
import time
from datetime import date

from pynput import keyboard
from pynput import mouse

import wx
import wx.adv

TMP_PATH = os.path.join(tempfile.gettempdir(),
                        "atbswp-" + date.today().strftime("%Y%d%w"))


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

    """
    def write_mouse_action(self, engine="pyautogui", move="", suffix=""):
        # TODO: Refactor mouse actions
    """

    def write_keyboard_action(self, engine="pyautogui", move="", key=""):
        suffix = "(" + repr(key) + ")"
        if move == "keyDown":
            # Corner case: Multiple successive keyDown
            if move + suffix in self.capture[-1]:
                move = 'press'
                self.capture[-1] = engine + "." + move + suffix
        self.capture.append(engine + "." + move + suffix)

    def on_move(self, x, y):
        if not self.recording:
            return False
        b = time.perf_counter()
        timeout = int(b - self.last_time)
        if timeout > 0:
            self.capture.append(f"time.sleep({timeout})")
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
        timeout = int(b - self.last_time)
        if timeout > 0:
            self.capture.append(f"time.sleep({timeout})")
        self.last_time = b

        try:
            # Ignore presses on Fn key
            if key.char:
                self.write_keyboard_action(move='keyDown', key=key.char)

        except AttributeError:
            if key == keyboard.Key.alt:
                if platform.system() == "Darwin":
                    self.write_keyboard_action(move="keyDown", key='option')
                else:
                    self.write_keyboard_action(move="keyDown", key='alt')
            elif key == keyboard.Key.alt_l:
                if platform.system() == "Darwin":
                    self.write_keyboard_action(move="keyDown", key='optionleft')
                else:
                    self.write_keyboard_action(move='keyDown', key='altleft')
            elif key == keyboard.Key.alt_r:
                if platform.system() == "Darwin":
                    self.write_keyboard_action(move='keyDown', key='optionright')
                else:
                    self.write_keyboard_action(move='keyDown', key='altright')
            elif key == keyboard.Key.alt_gr:
                self.write_keyboard_action(move='keyDown', key='altright')
            elif key == keyboard.Key.backspace:
                self.write_keyboard_action(move='keyDown', key='backspace')
            elif key == keyboard.Key.caps_lock:
                self.write_keyboard_action(move='keyDown', key='capslock')
            elif key == keyboard.Key.cmd:
                if platform.system() == "Darwin":
                    self.write_keyboard_action(move='keyDown', key='command')
                else:
                    self.write_keyboard_action(move='keyDown', key='winleft')
            elif key == keyboard.Key.cmd_r:
                if platform.system() == "Darwin":
                    self.write_keyboard_action(move='keyDown', key='cmdright')
                else:
                    self.write_keyboard_action(move='keyDown', key='winright')
            elif key == keyboard.Key.ctrl:
                self.write_keyboard_action(move='keyDown', key='ctrlleft')
            elif key == keyboard.Key.ctrl_r:
                self.write_keyboard_action(move='keyDown', key='ctrlright')
            elif key == keyboard.Key.delete:
                self.write_keyboard_action(move='keyDown', key='delete')
            elif key == keyboard.Key.down:
                self.write_keyboard_action(move='keyDown', key='down')
            elif key == keyboard.Key.end:
                self.write_keyboard_action(move='keyDown', key='end')
            elif key == keyboard.Key.enter:
                self.write_keyboard_action(move='keyDown', key='enter')
            elif key == keyboard.Key.esc:
                self.write_keyboard_action(move='keyDown', key='esc')
            elif key == keyboard.Key.f1:
                self.write_keyboard_action(move='keyDown', key='f1')
            elif key == keyboard.Key.f2:
                self.write_keyboard_action(move='keyDown', key='f2')
            elif key == keyboard.Key.f3:
                self.write_keyboard_action(move='keyDown', key='f3')
            elif key == keyboard.Key.f4:
                self.write_keyboard_action(move='keyDown', key='f4')
            elif key == keyboard.Key.f5:
                self.write_keyboard_action(move='keyDown', key='f5')
            elif key == keyboard.Key.f6:
                self.write_keyboard_action(move='keyDown', key='f6')
            elif key == keyboard.Key.f7:
                self.write_keyboard_action(move='keyDown', key='f7')
            elif key == keyboard.Key.f8:
                self.write_keyboard_action(move='keyDown', key='f8')
            elif key == keyboard.Key.f9:
                self.write_keyboard_action(move='keyDown', key='f9')
            elif key == keyboard.Key.f10:
                self.write_keyboard_action(move='keyDown', key='f10')
            elif key == keyboard.Key.f11:
                self.write_keyboard_action(move='keyDown', key='f11')
            elif key == keyboard.Key.f12:
                self.write_keyboard_action(move='keyDown', key='f12')
            elif key == keyboard.Key.home:
                self.write_keyboard_action(move='keyDown', key='home')
            elif key == keyboard.Key.left:
                self.write_keyboard_action(move='keyDown', key='left')
            elif key == keyboard.Key.page_down:
                self.write_keyboard_action(move='keyDown', key='pagedown')
            elif key == keyboard.Key.page_up:
                self.write_keyboard_action(move='keyDown', key='pageup')
            elif key == keyboard.Key.right:
                self.write_keyboard_action(move='keyDown', key='right')
            elif key == keyboard.Key.shift:
                self.write_keyboard_action(move='keyDown', key='shift_left')
            elif key == keyboard.Key.shift_r:
                self.write_keyboard_action(move='keyDown', key='shiftright')
            elif key == keyboard.Key.space:
                self.write_keyboard_action(move='keyDown', key='space')
            elif key == keyboard.Key.tab:
                self.write_keyboard_action(move='keyDown', key='tab')
            elif key == keyboard.Key.up:
                self.write_keyboard_action(move='keyDown', key='up')
            elif key == keyboard.Key.media_play_pause:
                self.write_keyboard_action(move='keyDown', key='playpause')
            elif key == keyboard.Key.insert:
                self.write_keyboard_action(move='keyDown', key='insert')
            elif key == keyboard.Key.menu:
                self.capture.append(f"### The menu key is not handled yet")
            elif key == keyboard.Key.num_lock:
                self.write_keyboard_action(move='keyDown', key='num_lock')
            elif key == keyboard.Key.pause:
                self.write_keyboard_action(move='keyDown', key='pause')
            elif key == keyboard.Key.print_screen:
                self.write_keyboard_action(move='keyDown', key='print_screen')
            elif key == keyboard.Key.scroll_lock:
                self.write_keyboard_action(move='keyDown', key='scroll_lock')
            else:
                self.capture.append(f"### {key} is not supported yet")

    def on_release(self, key):
        if not self.recording:
            return False
        if key == keyboard.Key.alt:
            if platform.system() == "Darwin":
                self.write_keyboard_action(move='keyUp', key='option')
            else:
                self.write_keyboard_action(move='keyUp', key='alt')
        elif key == keyboard.Key.alt_l:
            if platform.system() == "Darwin":
                self.write_keyboard_action(move='keyUp', key='optionleft')
            else:
                self.write_keyboard_action(move='keyUp', key='altleft')
        elif key == keyboard.Key.alt_r:
            if platform.system() == "Darwin":
                self.write_keyboard_action(move='keyUp', key='optionright')
            else:
                self.write_keyboard_action(move='keyUp', key='altright')
        elif key == keyboard.Key.alt_gr:
            self.write_keyboard_action(move='keyUp', key='altright')
        elif key == keyboard.Key.backspace:
            self.write_keyboard_action(move='keyUp', key='backspace')
        elif key == keyboard.Key.caps_lock:
            self.write_keyboard_action(move='keyUp', key='capslock')
        elif key == keyboard.Key.cmd:
            if platform.system() == "Darwin":
                self.write_keyboard_action(move='keyUp', key='command')
            else:
                self.write_keyboard_action(move='keyUp', key='winleft')
        elif key == keyboard.Key.cmd_r:
            if platform.system() == "Darwin":
                self.write_keyboard_action(move='keyUp', key='cmdright')
            else:
                self.write_keyboard_action(move='keyUp', key='winright')
        elif key == keyboard.Key.ctrl:
            self.write_keyboard_action(move='keyUp', key='ctrlleft')
        elif key == keyboard.Key.ctrl_r:
            self.write_keyboard_action(move='keyUp', key='ctrlright')
        elif key == keyboard.Key.delete:
            self.write_keyboard_action(move='keyUp', key='delete')
        elif key == keyboard.Key.down:
            self.write_keyboard_action(move='keyUp', key='down')
        elif key == keyboard.Key.end:
            self.write_keyboard_action(move='keyUp', key='end')
        elif key == keyboard.Key.enter:
            self.write_keyboard_action(move='keyUp', key='enter')
        elif key == keyboard.Key.esc:
            self.write_keyboard_action(move='keyUp', key='esc')
        elif key == keyboard.Key.f1:
            self.write_keyboard_action(move='keyUp', key='f1')
        elif key == keyboard.Key.f2:
            self.write_keyboard_action(move='keyUp', key='f2')
        elif key == keyboard.Key.f3:
            self.write_keyboard_action(move='keyUp', key='f3')
        elif key == keyboard.Key.f4:
            self.write_keyboard_action(move='keyUp', key='f4')
        elif key == keyboard.Key.f5:
            self.write_keyboard_action(move='keyUp', key='f5')
        elif key == keyboard.Key.f6:
            self.write_keyboard_action(move='keyUp', key='f6')
        elif key == keyboard.Key.f7:
            self.write_keyboard_action(move='keyUp', key='f7')
        elif key == keyboard.Key.f8:
            self.write_keyboard_action(move='keyUp', key='f8')
        elif key == keyboard.Key.f9:
            self.write_keyboard_action(move='keyUp', key='f9')
        elif key == keyboard.Key.f10:
            self.write_keyboard_action(move='keyUp', key='f10')
        elif key == keyboard.Key.f11:
            self.write_keyboard_action(move='keyUp', key='f11')
        elif key == keyboard.Key.f12:
            self.write_keyboard_action(move='keyUp', key='f12')
        elif key == keyboard.Key.home:
            self.write_keyboard_action(move='keyUp', key='home')
        elif key == keyboard.Key.left:
            self.write_keyboard_action(move='keyUp', key='left')
        elif key == keyboard.Key.page_down:
            self.write_keyboard_action(move='keyUp', key='pagedown')
        elif key == keyboard.Key.page_up:
            self.write_keyboard_action(move='keyUp', key='pageup')
        elif key == keyboard.Key.right:
            self.write_keyboard_action(move='keyUp', key='right')
        elif key == keyboard.Key.shift:
            self.write_keyboard_action(move='keyUp', key='shift_left')
        elif key == keyboard.Key.shift_r:
            self.write_keyboard_action(move='keyUp', key='shiftright')
        elif key == keyboard.Key.space:
            self.write_keyboard_action(move='keyUp', key='space')
        elif key == keyboard.Key.tab:
            self.write_keyboard_action(move='keyUp', key='tab')
        elif key == keyboard.Key.up:
            self.write_keyboard_action(move='keyUp', key='up')
        elif key == keyboard.Key.media_play_pause:
            self.write_keyboard_action(move='keyUp', key='playpause')
        elif key == keyboard.Key.insert:
            self.write_keyboard_action(move='keyUp', key='insert')
        elif key == keyboard.Key.menu:
            self.capture.append(f"### The menu key is not handled yet")
        elif key == keyboard.Key.num_lock:
            self.write_keyboard_action(move='keyUp', key='num_lock')
        elif key == keyboard.Key.pause:
            self.write_keyboard_action(move='keyUp', key='pause')
        elif key == keyboard.Key.print_screen:
            self.write_keyboard_action(move='keyUp', key='print_screen')
        elif key == keyboard.Key.scroll_lock:
            self.write_keyboard_action(move='keyUp', key='scroll_lock')
        elif key == keyboard._xorg.KeyCode(269025067): # Fn Key
            pass
        else:
            self.write_keyboard_action(move='keyUp', key=key)

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
