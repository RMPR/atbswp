"""Control actions triggered by the GUI."""

# atbswp: Record mouse and keyboard actions and reproduce them identically at will
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
import os
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

from custom_widgets import SliderDialog

import wx
import wx.adv
import wx.lib.newevent as NE


TMP_PATH = os.path.join(tempfile.gettempdir(),
                        "atbswp-" + date.today().strftime("%Y%m%d"))
HEADER = (
    f"#!/bin/env python3\n"
    f"# Created by atbswp v{settings.VERSION} "
    f"(https://git.sr.ht/~rmpr/atbswp)\n"
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
            coordinates = [int(s)
                           for s in parameters.split(", ") if isinteger(s)]
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
        timeout = float(b - self.last_time)
        if timeout > 0.0:
            self._capture.append(f"time.sleep({timeout})")
        self.last_time = b
        self.write_mouse_action(move="moveTo", parameters=f"{x}, {y}")

    def on_click(self, x, y, button, pressed):
        """Triggered by a mouse click."""
        if not self.recording:
            return False
        if pressed:
            if button == mouse.Button.left:
                self.write_mouse_action(
                    move="mouseDown", parameters=f"{x}, {y}, 'left'")
            elif button == mouse.Button.right:
                self.write_mouse_action(
                    move="mouseDown", parameters=f"{x}, {y}, 'right'")
            elif button == mouse.Button.middle:
                self.write_mouse_action(
                    move="mouseDown", parameters=f"{x}, {y}, 'middle'")
            else:
                wx.LogError("Mouse Button not recognized")
        else:
            if button == mouse.Button.left:
                self.write_mouse_action(
                    move="mouseUp", parameters=f"{x}, {y}, 'left'")
            elif button == mouse.Button.right:
                self.write_mouse_action(
                    move="mouseUp", parameters=f"{x}, {y}, 'right'")
            elif button == mouse.Button.middle:
                self.write_mouse_action(
                    move="mouseUp", parameters=f"{x}, {y}, 'middle'")
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
        timeout = float(b - self.last_time)
        if timeout > 0.0:
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
            current_value = settings.CONFIG.getint(
                'DEFAULT', 'Recording Timer')
        except:
            current_value = 0

        dialog = wx.NumberEntryDialog(None, message="Choose an amount of time (seconds)",
                                      prompt="", caption="Recording Timer", value=current_value, min=0, max=999)
        dialog.ShowModal()
        new_value = dialog.Value
        dialog.Destroy()
        settings.CONFIG['DEFAULT']['Recording Timer'] = str(new_value)

    def mouse_speed(event):
        """Set the mouse speed."""
        # Workaround for user upgrading from a previous version
        try:
            current_value = settings.CONFIG.getint(
                'DEFAULT', 'Mouse Speed')
        except:
            current_value = 21

        dialog = wx.NumberEntryDialog(None, message="Choose an amount of time (seconds)",
                                      prompt="", caption="Recording Timer", value=current_value, min=0, max=9999)
        dialog.ShowModal()
        new_value = dialog.Value
        dialog.Destroy()
        settings.CONFIG['DEFAULT']['Mouse Speed'] = str(new_value)

    def action(self, event):
        """Triggered when the recording button is clicked on the GUI."""
        self.mouse_sensibility = settings.CONFIG.getint("DEFAULT", "Mouse Speed")
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

        if event.EventObject.Value:
            if self.timer > 0:
                self.countdown_dialog = wx.ProgressDialog(title="Wait for the recording to start",
                                                          message=f"The recording will start in {self.timer} second(s)",
                                                          style=wx.PD_APP_MODAL | wx.PD_CAN_SKIP | wx.PD_SMOOTH)
                self.countdown_dialog.Range = self.timer
                self.wx_timer = wx.Timer(event.GetEventObject())
                event.EventObject.Bind(
                    wx.EVT_TIMER, self.update_timer, self.wx_timer)
                self.wx_timer.Start(1000)
                self.countdown_dialog.ShowModal()

            listener_keyboard.start()
            listener_mouse.start()
            self.last_time = time.perf_counter()
            self.recording = True
            recording_state = wx.Icon(os.path.join(
                self.path, "img", "icon-recording.png"))
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
            recording_state = wx.Icon(
                os.path.join(self.path, "img", "icon.png"))
        event.GetEventObject().GetParent().taskbar.SetIcon(recording_state)

    def update_timer(self, event):
        """Check if it's the time to start to record"""
        if self.timer <= 0 or self.countdown_dialog.WasSkipped():
            self.wx_timer.Stop()
            self.countdown_dialog.Destroy()
        else:
            time.sleep(1)
            self.timer -= 1
            self.countdown_dialog.Update(
                self.timer, f"The recording will start in {self.timer} second(s)")


class PlayCtrl:
    """Control class for the play button."""

    global TMP_PATH

    def __init__(self):
        self.count = settings.CONFIG.getint('DEFAULT', 'Repeat Count')
        self.infinite = settings.CONFIG.getboolean(
            'DEFAULT', 'Infinite Playback')
        self.count_was_updated = False
        self.ThreadEndEvent, self.EVT_THREAD_END = NE.NewEvent()

    def play(self, capture, toggle_button):
        """Play the loaded capture."""
        toggle_value = True
        for line in capture:
            if self.play_thread.ended():
                return
            exec(line)

        if self.count <= 0 and not self.infinite:
            toggle_value = False
        event = self.ThreadEndEvent(
            count=self.count, toggle_value=toggle_value)
        wx.PostEvent(toggle_button.Parent, event)

        btn_event = wx.CommandEvent(wx.wxEVT_TOGGLEBUTTON)
        btn_event.EventObject = toggle_button
        self.action(btn_event)

    def action(self, event):
        """Replay a `count` number of time."""
        toggle_button = event.GetEventObject()
        toggle_button.Parent.panel.SetFocus()
        self.infinite = settings.CONFIG.getboolean(
            'DEFAULT', 'Infinite Playback')
        if toggle_button.Value:
            if not self.count_was_updated:
                self.count = settings.CONFIG.getint('DEFAULT', 'Repeat Count')
                self.count_was_updated = True
            if TMP_PATH is None or not os.path.isfile(TMP_PATH):
                wx.LogError("No capture loaded")
                event = self.ThreadEndEvent(
                    count=self.count, toggle_value=False)
                wx.PostEvent(toggle_button.Parent, event)
                return
            with open(TMP_PATH, 'r') as f:
                capture = f.readlines()
            if self.count > 0 or self.infinite:
                self.count = self.count - 1 if not self.infinite else self.count
                self.play_thread = PlayThread()
                self.play_thread.daemon = True
                self.play_thread = PlayThread(target=self.play,
                                              args=(capture, toggle_button,))
                self.play_thread.start()
        else:
            self.play_thread.end()
            self.count_was_updated = False
            settings.save_config()


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
        current_value = settings.CONFIG.getboolean(
            'DEFAULT', 'Infinite Playback')
        settings.CONFIG['DEFAULT']['Infinite Playback'] = str(
            not current_value)

    def repeat_count(self, event):
        """Set the repeat count."""
        current_value = settings.CONFIG.getint('DEFAULT', 'Repeat Count')
        dialog = wx.NumberEntryDialog(None, message="Choose a repeat count",
                                      prompt="", caption="Repeat Count", value=current_value, min=1, max=999)
        dialog.ShowModal()
        new_value = str(dialog.Value)
        dialog.Destroy()
        settings.CONFIG['DEFAULT']['Repeat Count'] = new_value
        self.main_dialog.remaining_plays.Label = new_value

    @staticmethod
    def recording_hotkey(event):
        """Set the recording hotkey."""
        current_value = settings.CONFIG.getint('DEFAULT', 'Recording Hotkey')
        dialog = SliderDialog(None, title="Choose a function key: F2-12", size=(500, 50),
                              default_value=current_value-339, min_value=2, max_value=12)
        dialog.ShowModal()
        new_value = dialog.value + 339
        if new_value == settings.CONFIG.getint('DEFAULT', 'Playback Hotkey'):
            dlg = wx.MessageDialog(
                None, "Recording hotkey should be different from Playback one", "Error", wx.OK | wx.ICON_ERROR)
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
            dlg = wx.MessageDialog(
                None, "Playback hotkey should be different from Recording one", "Error", wx.OK | wx.ICON_ERROR)
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
        settings.CONFIG['DEFAULT']['Language'] = item.GetItemLabelText()
        settings.save_config()
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
