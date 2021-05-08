"""Control actions triggered by the GUI."""

# atbswp: Record mouse and keyboard actions and reproduce them identically at will
#
# Copyright (C) 2019 Mairo Rufus <github@rmpr.xyz>
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
from threading import Event
from threading import Thread

import pyautogui

from pynput import keyboard, mouse

import settings

from custom_widgets import SliderDialog, CountdownDialog

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
        event.EventObject.Parent.panel.SetFocus()
        dlg.Destroy()

    def save_file(self, event):
        """Save the capture currently loaded."""
        event.EventObject.Parent.panel.SetFocus()

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
            if abs(coordinates[0] - self._lastx) < self.mouse_sensibility \
               and abs(coordinates[1] - self._lasty) < self.mouse_sensibility:
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

    def recording_timer(event):
        """Set the recording timer."""
        # Workaround for user upgrading from a previous version
        try:
            current_value = settings.CONFIG.getint('DEFAULT', 'Recording Timer')
        except:
            current_value = 0

        dialog = SliderDialog(None, title="Choose an amount of time (seconds)", size=(500, 50), default_value=current_value)
        dialog.ShowModal()
        new_value = dialog.value
        dialog.Destroy()
        settings.CONFIG['DEFAULT']['Recording Timer'] = str(new_value)

    def action(self, event):
        """Triggered when the recording button is clicked on the GUI."""
        listener_mouse = mouse.Listener(
            on_move=self.on_move,
            on_click=self.on_click,
            on_scroll=self.on_scroll)
        listener_keyboard = keyboard.Listener(
            on_press=self.on_press,
            on_release=self.on_release)

        try:
            self.timer = settings.CONFIG.getint("DEFAULT", "Recording Timer")
        except:
            self.timer = 0

        if event.GetEventObject().GetValue():
            if self.timer > 0:
                self.countdown_dialog = CountdownDialog(None, title="Wait for the recording to start", countdown=self.timer)
                self.wx_timer = wx.Timer(event.GetEventObject())
                event.GetEventObject().Bind(wx.EVT_TIMER, self.update_timer, self.wx_timer)
                self.wx_timer.Start(1000)
                self.countdown_dialog.ShowModal()

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
                self._capture.pop()
                f.seek(0)
                f.write("\n".join(self._capture))
                f.truncate()
            self._capture = [self._header]
            recording_state = wx.Icon(os.path.join(self.path, "img", "icon.png"))
        event.GetEventObject().GetParent().taskbar.SetIcon(recording_state)

    def update_timer(self, event):
        """Check if it's the time to start to record"""
        if self.timer <= 0:
            self.wx_timer.Stop()
            self.countdown_dialog.Destroy()
        else:
            self.timer = self.countdown_dialog.update_ui()


class PlayControl:
    """Control class for playback functionality"""

    def __init__(self, count=1, infinite=False):
        self.count = count
        self.infinite = infinite
        self._stop_locks = [True]
        self._current_count = 0
        self._play_thread = None

    def _play(self, capture, append_function=None):
        for line in capture:
            if self.is_stoped(): break
            exec(line)
        self._current_count += 1
        if self._current_count < self.count or self.infinite \
                and not self.is_stoped():
            self._play(capture, append_function)
        else:
            self.stop()
            if append_function:
                append_function()

    def _start_thread(self, target, *args, daemon=False):
        thread = Thread(target=target, args=(*args,))
        thread.daemon = daemon
        thread.start()
        return thread

    def set_config(self, count, infinite):
        """Set the configuration to the next playback"""
        self.count = count
        self.infinite = infinite

    def get_current_count(self):
        """Returns the current playback time"""
        return self._current_count

    def is_stoped(self):
        return self._stop_locks[0]

    def play(self, capture, append_function=None):
        """Starts playback according with configs setted

            :param capture: list of commands to be executed
            :param append_function: a function to be executed after
             a playback
        """
        self._stop_locks[0] = False
        self._current_count = 0
        self._play_thread = self._start_thread(self._play, capture,
                                                append_function)

    def stop(self):
        self._stop_locks[0] = True


class PlayGui:
    def __init__(self):
        self.play_ctrl = PlayControl()
        self.capture = None
        self._config_was_updated = False
        self._toggle_button = None

    def _load_capture_file(self):
        if TMP_PATH is None or not os.path.isfile(TMP_PATH):
            wx.LogError("No capture loaded")
            self._toggle_button.Value = False
            return False
        with open(TMP_PATH, 'r') as f:
            self.capture = f.readlines()
            return True

    def _set_config(self):
        if self._config_was_updated: return
        count = settings.CONFIG.getint('DEFAULT', 'Repeat Count')
        infinite = settings.CONFIG.getboolean('DEFAULT', 'Infinite Playback')
        self.play_ctrl.set_config(count, infinite)
        self._config_was_updated = False

    def _stop(self):
        self._toggle_button.Value = False
        self.play_ctrl.stop()
        self._config_was_updated = False
        settings.save_config()

    def action(self, event):
        self._toggle_button = event.GetEventObject()
        self._toggle_button.Parent.panel.SetFocus()
        if self._toggle_button.Value:
            if self.play_ctrl.is_stoped():
                if not self._load_capture_file(): return
                self._set_config()
                self.play_ctrl.play(self.capture, self._stop)
        else:
            self._stop()


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
        event.EventObject.Parent.panel.SetFocus()
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
        dialog = wx.NumberEntryDialog(None, message="Choose a repeat count", prompt="", caption="Repeat Count", value=current_value, min=1, max=999)
        dialog.ShowModal()
        new_value = dialog.Value
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

class PlayThread(Thread):
    """Thread with an end method triggered by a click on the Toggle button."""
    def __init__(self, *args, **kwargs):
        super(PlayThread, self).__init__(*args, **kwargs)
        self._end = Event()

    def end(self):
        self._end.set()

    def ended(self):
        return self._end.isSet()


