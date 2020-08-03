"""Control actions triggered by the GUI."""

# atbswp: Record mouse and keyboard actions and reproduce them identically at will
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
import sys
import tempfile
import time
from datetime import date
from pathlib import Path
from threading import Thread

import pyautogui

from pynput import keyboard, mouse

import settings

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

LOOKUP_SPECIAL_KEY = {}


class FileChooserCtrl:
    """Control class for both the open capture and save capture options.

    Keyword arguments:
    capture -- content of the temporary file
    parent -- the parent Frame
    """

    capture = str()

    def __init__(self, parent):
        """Set the parent frame."""
        self.parent = parent

    def load_content(self, path):
        """Load the temp file capture from disk."""
        # TODO: Better input control
        if not path or not os.path.isfile(path):
            return None
        with open(path, 'r') as f:
            return f.read()

    def load_file(self, event):
        """Load a capture manually chosen by the user."""
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
        """Save the capture currently loaded."""
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
    """Control class for the record button.

    Keyword arguments:
    capture -- current recording
    mouse_sensibility -- granularity for mouse capture
    """

    def __init__(self):
        """Initialize a new record."""
        self._header = HEADER
        self._error = "### This key is not supported yet"

        self._capture = [self._header]
        self._lastx, self._lasty = pyautogui.position()
        self.mouse_sensibility = 21
        if getattr(sys, 'frozen', False):
            self.path = sys._MEIPASS
        else:
            self.path = Path(__file__).parent.absolute()

        LOOKUP_SPECIAL_KEY[keyboard.Key.alt] = 'alt'
        LOOKUP_SPECIAL_KEY[keyboard.Key.alt_l] = 'altleft'
        LOOKUP_SPECIAL_KEY[keyboard.Key.alt_r] = 'altright'
        LOOKUP_SPECIAL_KEY[keyboard.Key.alt_gr] = 'altright'
        LOOKUP_SPECIAL_KEY[keyboard.Key.backspace] = 'backspace'
        LOOKUP_SPECIAL_KEY[keyboard.Key.caps_lock] = 'capslock'
        LOOKUP_SPECIAL_KEY[keyboard.Key.cmd] = 'winleft'
        LOOKUP_SPECIAL_KEY[keyboard.Key.cmd_r] = 'winright'
        LOOKUP_SPECIAL_KEY[keyboard.Key.ctrl] = 'ctrlleft'
        LOOKUP_SPECIAL_KEY[keyboard.Key.ctrl_r] = 'ctrlright'
        LOOKUP_SPECIAL_KEY[keyboard.Key.delete] = 'delete'
        LOOKUP_SPECIAL_KEY[keyboard.Key.down] = 'down'
        LOOKUP_SPECIAL_KEY[keyboard.Key.end] = 'end'
        LOOKUP_SPECIAL_KEY[keyboard.Key.enter] = 'enter'
        LOOKUP_SPECIAL_KEY[keyboard.Key.esc] = 'esc'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f1] = 'f1'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f2] = 'f2'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f3] = 'f3'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f4] = 'f4'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f5] = 'f5'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f6] = 'f6'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f7] = 'f7'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f8] = 'f8'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f9] = 'f9'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f10] = 'f10'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f11] = 'f11'
        LOOKUP_SPECIAL_KEY[keyboard.Key.f12] = 'f12'
        LOOKUP_SPECIAL_KEY[keyboard.Key.home] = 'home'
        LOOKUP_SPECIAL_KEY[keyboard.Key.left] = 'left'
        LOOKUP_SPECIAL_KEY[keyboard.Key.page_down] = 'pagedown'
        LOOKUP_SPECIAL_KEY[keyboard.Key.page_up] = 'pageup'
        LOOKUP_SPECIAL_KEY[keyboard.Key.right] = 'right'
        LOOKUP_SPECIAL_KEY[keyboard.Key.shift] = 'shift_left'
        LOOKUP_SPECIAL_KEY[keyboard.Key.shift_r] = 'shiftright'
        LOOKUP_SPECIAL_KEY[keyboard.Key.space] = 'space'
        LOOKUP_SPECIAL_KEY[keyboard.Key.tab] = 'tab'
        LOOKUP_SPECIAL_KEY[keyboard.Key.up] = 'up'
        LOOKUP_SPECIAL_KEY[keyboard.Key.media_play_pause] = 'playpause'
        LOOKUP_SPECIAL_KEY[keyboard.Key.insert] = 'insert'
        LOOKUP_SPECIAL_KEY[keyboard.Key.num_lock] = 'num_lock'
        LOOKUP_SPECIAL_KEY[keyboard.Key.pause] = 'pause'
        LOOKUP_SPECIAL_KEY[keyboard.Key.print_screen] = 'print_screen'
        LOOKUP_SPECIAL_KEY[keyboard.Key.scroll_lock] = 'scroll_lock'

    def write_mouse_action(self, engine="pyautogui", move="", parameters=""):
        """Append a new mouse move to capture.

        Keyword arguments:
        engine -- the replay library used (default pyautogui)
        move -- the mouse movement (mouseDown, mouseUp, scroll, moveTo)
        parameters -- the details of the movement
        """
        def isinteger(s):
            try:
                int(s)
                return True
            except:
                return False

        if move == "moveTo":
            coordinates = [int(s) for s in parameters.split(", ") if isinteger(s)]
            if coordinates[0] - self._lastx < self.mouse_sensibility \
               and coordinates[1] - self._lasty < self.mouse_sensibility:
                return
            else:
                self._lastx, self._lasty = coordinates
        self._capture.append(engine + "." + move + '(' + parameters + ')')

    def write_keyboard_action(self, engine="pyautogui", move="", key=""):
        """Append keyboard actions to the class variable capture.

        Keyword arguments:
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
        """Triggered by a mouse move."""
        if not self.recording:
            return False
        b = time.perf_counter()
        timeout = int(b - self.last_time)
        if timeout > 0:
            self._capture.append(f"time.sleep({timeout})")
        self.last_time = b
        self.write_mouse_action(move="moveTo", parameters=f"{x}, {y}")

    def on_click(self, x, y, button, pressed):
        """Triggered by a mouse click."""
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
        """Triggered by a mouse wheel scroll."""
        if not self.recording:
            return False
        self.write_mouse_action(move="scroll", parameters=f"{y}")

    def on_press(self, key):
        """Triggered by a key press."""
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
            self.write_keyboard_action(move="keyDown",
                                       key=LOOKUP_SPECIAL_KEY.get(key,
                                       self._error))

    def on_release(self, key):
        """Triggered by a key released."""
        if not self.recording:
            return False
        else:
            if len(str(key)) <= 3:
                self.write_keyboard_action(move='keyUp', key=key)
            else:
                self.write_keyboard_action(move="keyUp",
                                           key=LOOKUP_SPECIAL_KEY.get(key,
                                           self._error))

    def action(self, event):
        """Triggered when the recording button is clicked on the GUI."""
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
    """Control class for the play button."""

    global TMP_PATH

    def play(self, capture, toggle_button):
        """Play the loaded capture."""
        toggle_button.Value = True
        exec(capture)
        toggle_button.Value = False

    def action(self, event):
        """Replay a `count` number of time."""
        toggle_button = event.GetEventObject()
        count = settings.CONFIG.getint('DEFAULT', 'Repeat Count')
        infinite = settings.CONFIG.getboolean('DEFAULT', 'Infinite Playback')
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
                self.play_thread = Thread()
                self.play_thread.daemon = True
                self.play_thread = Thread(target=self.play,
                                          args=(capture, toggle_button,))
                self.play_thread.start()
            else:
                i = 1
                while i <= count or infinite:
                    self.play_thread = Thread()
                    self.play_thread = Thread(target=self.play,
                                              args=(capture, toggle_button,))
                    self.play_thread.start()
                    self.play_thread.join()
                    i += 1
        else:
            if getattr(sys, 'frozen', False):
                path = os.path.join(Path(__file__).parent.absolute(), 'atbswp')
            else:
                path = os.path.join(Path(__file__).parent.absolute(), 'atbswp.py')
            settings.save_config()
            os.execl(path)


class CompileCtrl:
    """Produce an executable Python bytecode file."""

    @staticmethod
    def compile(event):
        """Return a compiled version of the capture currently loaded.

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
    """Control class for the settings."""

    def __init__(self, main_dialog):
        """Copy the reference of the main Window."""
        self.main_dialog = main_dialog

    @staticmethod
    def playback_speed(event):
        """Replay the capture 2 times faster."""
        # TODO: To implement
        pass

    @staticmethod
    def infinite_playback(event):
        """Toggle infinite playback."""
        current_value = settings.CONFIG.getboolean('DEFAULT', 'Infinite Playback')
        settings.CONFIG['DEFAULT']['Infinite Playback'] = str(not current_value)

    @staticmethod
    def repeat_count(event):
        """Set the repeat count."""
        current_value = settings.CONFIG.getint('DEFAULT', 'Repeat Count')
        dialog = SliderDialog(None, title="Choose a repeat count", size=(500, 50), default_value=current_value)
        dialog.ShowModal()
        new_value = dialog.value
        dialog.Destroy()
        settings.CONFIG['DEFAULT']['Repeat Count'] = str(new_value)

    @staticmethod
    def recording_hotkey(event):
        """Set the recording hotkey."""
        current_value = settings.CONFIG.getint('DEFAULT', 'Recording Hotkey')
        dialog = SliderDialog(None, title="Choose a function key: F2-12", size=(500, 50),
                              default_value=current_value-339, min_value=2, max_value=12)
        dialog.ShowModal()
        new_value = dialog.value + 339
        if new_value == settings.CONFIG.getint('DEFAULT', 'Playback Hotkey'):
            dlg = wx.MessageDialog(None, "Recording hotkey should be different from Playback one", "Error", wx.OK | wx.ICON_ERROR)
            dlg.ShowModal()
            dlg.Destroy()
        dialog.Destroy()
        settings.CONFIG['DEFAULT']['Recording Hotkey'] = str(new_value)

    @staticmethod
    def playback_hotkey(event):
        """Set the playback hotkey."""
        current_value = settings.CONFIG.getint('DEFAULT', 'Playback Hotkey')
        dialog = SliderDialog(None, title="Choose a function key: F2-12", size=(500, 50),
                              default_value=current_value-339, min_value=2, max_value=12)
        dialog.ShowModal()
        new_value = dialog.value + 339
        if new_value == settings.CONFIG.getint('DEFAULT', 'Recording Hotkey'):
            dlg = wx.MessageDialog(None, "Playback hotkey should be different from Recording one", "Error", wx.OK | wx.ICON_ERROR)
            dlg.ShowModal()
            dlg.Destroy()
        dialog.Destroy()
        settings.CONFIG['DEFAULT']['Playback Hotkey'] = str(new_value)

    def always_on_top(self, event):
        """Toggle the always on top setting."""
        current_value = settings.CONFIG['DEFAULT']['Always On Top']
        style = self.main_dialog.GetWindowStyle()
        self.main_dialog.SetWindowStyle(style ^ wx.STAY_ON_TOP)
        settings.CONFIG['DEFAULT']['Always On Top'] = str(not current_value)

    def language(self, event):
        """Manage the language among the one available."""
        menu = event.EventObject
        item = menu.FindItemById(event.Id)
        settings.CONFIG['DEFAULT']['Language'] = item.Name
        dialog = wx.MessageDialog(None,
                                  message="Restart the program to apply modifications",
                                  pos=wx.DefaultPosition)
        dialog.ShowModal()


class HelpCtrl:
    """Control class for the About menu."""

    @staticmethod
    def action(event):
        """Open the browser on the quick demo of atbswp"""
        url = "https://youtu.be/L0jjSgX5FYk"
        wx.LaunchDefaultBrowser(url, flags=0)


class SliderDialog(wx.Dialog):
    """Wrap a slider in a dialog and get the value"""

    def __init__(self, *args, **kwargs):
        """Initialize the widget with the custom attributes."""
        self._current_value = kwargs.pop("default_value", 1)
        self.min_value = kwargs.pop("min_value", 1)
        self.max_value = kwargs.pop("max_value", 999)
        super(SliderDialog, self).__init__(*args, **kwargs)
        self._value = 1
        self.init_ui()
        self.Bind(wx.EVT_CLOSE, self.on_close)

    def init_ui(self):
        """Initialize the UI elements"""
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
        """Triggered when the widget is closed."""
        self._value = self.slider.Value
        event.Skip()

    @property
    def value(self):
        """Getter."""
        return self._value

    @value.setter
    def value(self, value):
        """Setter."""
        self._value = value
