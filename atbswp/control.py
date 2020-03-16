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
import py_compile
import shutil
import tempfile
import time
from datetime import date
from pathlib import Path
from threading import Thread

import utils

import pyautogui

from pynput import keyboard
from pynput import mouse

import wx
import wx.adv


TMP_PATH = os.path.join(tempfile.gettempdir(),
                        "atbswp-" + date.today().strftime("%Y%m%d"))
HEADER = (
            f"#!/bin/env python3\n"
            f"# Created by atbswp (https://github.com/rmpr/atbswp)\n"
            f"# on {date.today().strftime('%d %b %Y ')}\n"
            f"import pyautogui\n"
            f"import time\n"
            f"pyautogui.FAILSAFE = False\n"
        )


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
        title = "Choose a capture file:"
        dlg = wx.FileDialog(self.parent,
                            message=title,
                            defaultDir="~",
                            defaultFile="capture.py",
                            style=wx.DD_DEFAULT_STYLE)
        if dlg.ShowModal() == wx.ID_OK:
            self._capture = self.load_content(dlg.GetPath())
            with open(TMP_PATH, 'w') as f:
                f.write(self._capture)
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
        self._header = HEADER

        self._capture = [self._header]
        self._lastx, self._lasty = pyautogui.position()
        self.mouse_sensibility = 21
        self.path = Path(__file__).parent.absolute()

    def write_mouse_action(self, engine="pyautogui", move="", parameters=""):
        if move == "moveTo":
            coordinates = [int(s) for s in parameters.split(", ") if s.isdigit()]
            if coordinates[0] - self._lastx < self.mouse_sensibility \
               and coordinates[1] - self._lasty < self.mouse_sensibility:
                return
            else:
                self._lastx, self._lasty = coordinates
        self._capture.append(engine + "." + move + '(' + parameters + ')')

    def write_keyboard_action(self, engine="pyautogui", move="", key=""):
        """
        Append keyboard actions to the class variable capture
        Keyword Arguments:
        - engine: the module which will be used for the replay
        - move: keyDown | keyUp
        - key: The key pressed
        """
        suffix = "(" + repr(key) + ")"
        if move == "keyDown":
            # Corner case: Multiple successive keyDown
            if move + suffix in self._capture[-1]:
                move = 'press'
                self._capture[-1] = engine + "." + move + suffix
        self._capture.append(engine + "." + move + suffix)

    def on_move(self, x, y):
        if not self.recording:
            return False
        b = time.perf_counter()
        timeout = int(b - self.last_time)
        if timeout > 0:
            self._capture.append(f"time.sleep({timeout})")
        self.last_time = b
        self.write_mouse_action(move="moveTo", parameters=f"{x}, {y}")

    def on_click(self, x, y, button, pressed):
        if not self.recording:
            return False
        if pressed:
            if button == mouse.Button.left:
                self.write_mouse_action(move="mouseDown", parameters=f"{x}, {y}, 'left'")
            elif button == mouse.Button.right:
                self.write_mouse_action(move="mouseDown", parameters=f"{x}, {y}, 'right'")
            elif button == mouse.Button.middle:
                self.write_mouse_action(move="mouseDown", parameters=f"{x}, {y}, 'middle'")
            else:
                wx.LogError("Mouse Button not recognized")
        else:
            if button == mouse.Button.left:
                self.write_mouse_action(move="mouseUp", parameters=f"{x}, {y}, 'left'")
            elif button == mouse.Button.right:
                self.write_mouse_action(move="mouseUp", parameters=f"{x}, {y}, 'right'")
            elif button == mouse.Button.middle:
                self.write_mouse_action(move="mouseUp", parameters=f"{x}, {y}, 'middle'")
            else:
                wx.LogError("Mouse Button not recognized")

    def on_scroll(self, x, y, dx, dy):
        if not self.recording:
            return False
        self.write_mouse_action(move="scroll", parameters=f"{y}")

    def on_press(self, key):
        if not self.recording:
            return False
        b = time.perf_counter()
        timeout = int(b - self.last_time)
        if timeout > 0:
            self._capture.append(f"time.sleep({timeout})")
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
                self._capture.append(f"### The menu key is not handled yet")
            elif key == keyboard.Key.num_lock:
                self.write_keyboard_action(move='keyDown', key='num_lock')
            elif key == keyboard.Key.pause:
                self.write_keyboard_action(move='keyDown', key='pause')
            elif key == keyboard.Key.print_screen:
                self.write_keyboard_action(move='keyDown', key='print_screen')
            elif key == keyboard.Key.scroll_lock:
                self.write_keyboard_action(move='keyDown', key='scroll_lock')
            else:
                self._capture.append(f"### {key} is not supported yet")

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
            self._capture.append(f"### The menu key is not handled yet")
        elif key == keyboard.Key.num_lock:
            self.write_keyboard_action(move='keyUp', key='num_lock')
        elif key == keyboard.Key.pause:
            self.write_keyboard_action(move='keyUp', key='pause')
        elif key == keyboard.Key.print_screen:
            self.write_keyboard_action(move='keyUp', key='print_screen')
        elif key == keyboard.Key.scroll_lock:
            self.write_keyboard_action(move='keyUp', key='scroll_lock')
        else:
            if len(str(key)) <= 3:
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
            recording_state = wx.Icon(os.path.join(self.path, "img", "icon-recording.png"))
        else:
            self.recording = False
            with open(TMP_PATH, 'w') as f:
                # Remove the recording trigger event
                self._capture.pop()
                # If it's the mouse remove the previous also
                if "mouseDown" in self._capture[-1]:
                    self._capture.pop()
                f.seek(0)
                f.write("\n".join(self._capture))
                f.truncate()
            self._capture = [self._header]
            recording_state = wx.Icon(os.path.join(self.path, "img", "icon.png"))
        event.GetEventObject().GetParent().taskbar.SetIcon(recording_state)


class PlayCtrl:
    """
    Control class for the play button
    """
    global TMP_PATH

    def __init__(self):
        pass

    def play(self, capture, toggle_button):
        toggle_button.Value = True
        exec(capture)
        toggle_button.Value = False

    def action(self, event):
        """
         Replay a count number of time.
        """
        toggle_button = event.GetEventObject()
        count = utils.CONFIG.getint('DEFAULT', 'Repeat Count')
        infinite = utils.CONFIG.getboolean('DEFAULT', 'Infinite Playback')
        if toggle_button.Value:
            if TMP_PATH is None or not os.path.isfile(TMP_PATH):
                wx.LogError("No capture loaded")
                toggle_button.Value = False
                return
            with open(TMP_PATH, 'r') as f:
                capture = f.read()
            if capture == HEADER:
                wx.LogError("Empty capture")
                toggle_button.Value = False
                return
            if count == 1 and not infinite:
                    play_thread = Thread()
                    play_thread.daemon = True
                    play_thread = Thread(target=self.play,
                                         args=(capture, toggle_button,))
                    play_thread.start()
            else:
                i = 1
                while i <= count or infinite:
                    play_thread = Thread()
                    play_thread = Thread(target=self.play,
                                         args=(capture, toggle_button,))
                    play_thread.start()
                    play_thread.join()
                    i += 1
        else:
            play_thread._stop()  # Can be deprecated
            toggle_button.Value = False


class CompileCtrl:
    @staticmethod
    def compile(event):
        """
        Return a compiled version of the capture currently loaded
        For now it only returns a bytecode file.
        #TODO: Return a proper executable for the platform currently
        used **Without breaking the current workflow** which works both
        in development mode and in production
        """
        try:
            bytecode_path = py_compile.compile(TMP_PATH)
        except:
            wx.LogError("No capture loaded")
            return
        default_file = "capture.pyc"
        with wx.FileDialog(parent=event.GetEventObject().Parent, message="Save capture executable",
                           defaultDir=os.path.expanduser("~"), defaultFile=default_file, wildcard="*",
                           style=wx.FD_SAVE | wx.FD_OVERWRITE_PROMPT) as fileDialog:

            if fileDialog.ShowModal() == wx.ID_CANCEL:
                return     # the user changed their mind
            pathname = fileDialog.GetPath()
            try:
                shutil.copy(bytecode_path, pathname)
            except IOError:
                wx.LogError(f"Cannot save current data in file {pathname}.")


class SettingsCtrl:
    """
    Control class for the settings
    """
    def __init__(self, main_dialog):
        self.main_dialog = main_dialog

    @staticmethod
    def playback_speed(event):
        # TODO: To implement
        pass

    @staticmethod
    def infinite_playback(event):
        current_value = utils.CONFIG.getboolean('DEFAULT', 'Infinite Playback')
        utils.CONFIG['DEFAULT']['Infinite Playback'] = str(not current_value)

    @staticmethod
    def repeat_count(event):
        current_value = utils.CONFIG.getint('DEFAULT', 'Repeat Count')
        dialog = SliderDialog(None, title="Choose a repeat count", size=(500, 50), default_value=current_value)
        dialog.ShowModal()
        new_value = dialog.value
        dialog.Destroy()
        utils.CONFIG['DEFAULT']['Repeat Count'] = str(new_value)

    @staticmethod
    def recording_hotkey(event):
        current_value = utils.CONFIG.getint('DEFAULT', 'Recording Hotkey')
        dialog = SliderDialog(None, title="Choose a function key: F2-12", size=(500, 50),
                              default_value=current_value-339, min_value=2, max_value=12)
        dialog.ShowModal()
        new_value = dialog.value + 339
        if new_value == utils.CONFIG.getint('DEFAULT', 'Playback Hotkey'):
            dlg = wx.MessageDialog(None, "Recording hotkey should be different from Playback one", "Error", wx.OK|wx.ICON_ERROR)
            dlg.ShowModal()
            dlg.Destroy()
        dialog.Destroy()
        utils.CONFIG['DEFAULT']['Recording Hotkey'] = str(new_value)

    @staticmethod
    def playback_hotkey(event):
        current_value = utils.CONFIG.getint('DEFAULT', 'Playback Hotkey')
        dialog = SliderDialog(None, title="Choose a function key: F2-12", size=(500, 50),
                              default_value=current_value-339, min_value=2, max_value=12)
        dialog.ShowModal()
        new_value = dialog.value + 339
        if new_value == utils.CONFIG.getint('DEFAULT', 'Recording Hotkey'):
            dlg = wx.MessageDialog(None, "Playback hotkey should be different from Recording one", "Error", wx.OK|wx.ICON_ERROR)
            dlg.ShowModal()
            dlg.Destroy()
        dialog.Destroy()
        utils.CONFIG['DEFAULT']['Playback Hotkey'] = str(new_value)

    def always_on_top(self, event):
        current_value = utils.CONFIG['DEFAULT']['Always On Top']
        style = self.main_dialog.GetWindowStyle()
        self.main_dialog.SetWindowStyle(style ^ wx.STAY_ON_TOP)
        utils.CONFIG['DEFAULT']['Always On Top'] = str(not current_value)


class HelpCtrl:
    """
    Control class for the About menu
    """
    @staticmethod
    def action(event):
        url = "https://youtu.be/L0jjSgX5FYk"
        wx.LaunchDefaultBrowser(url, flags=0)


class SliderDialog(wx.Dialog):
    """
    Wrap a slider in a dialog and get the value
    """

    def __init__(self, *args, **kwargs):
        self._current_value = kwargs.pop("default_value", 1)
        self.min_value = kwargs.pop("min_value", 1)
        self.max_value = kwargs.pop("max_value", 999)
        super(SliderDialog, self).__init__(*args, **kwargs)
        self._value = 1
        self.init_ui()
        self.Bind(wx.EVT_CLOSE, self.on_close)

    def init_ui(self):
        pnl = wx.Panel(self)
        sizer = wx.BoxSizer(orient=wx.HORIZONTAL)
        self.slider = wx.Slider(parent=pnl, id=wx.ID_ANY, value=self._current_value,
                           minValue=self.min_value, maxValue=self.max_value,
                           name="Choose a number", size=self.GetSize(),
                           style=wx.SL_VALUE_LABEL | wx.SL_AUTOTICKS)
        sizer.Add(self.slider)
        pnl.SetSizer(sizer)
        sizer.Fit(self)

    def on_close(self, event):
        self._value = self.slider.Value
        event.Skip()

    @property
    def value(self):
        return self._value

    @value.setter
    def value(self, value):
        self._value = value

