#!/usr/bin/env python3
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
# Handle the config file of the program

import configparser
import os
import platform
from datetime import date


CONFIG = configparser.ConfigParser()
VERSION = "0.3.1"
YEAR = date.today().strftime("%Y")


# Check the location of the configuration file, default to the home directory
filename = "atbswp.cfg"
if platform.system() == "Linux":
    config_location = os.path.join(os.environ.get("HOME"), ".config")
elif platform.system() == "Windows":
    config_location = os.environ.get("APPDATA")
else:
    config_location = os.environ.get("HOME")

config_location = os.path.join(config_location, filename)


def save_config():
    with open(config_location, "w") as config_file:
        CONFIG.write(config_file)


try:
    with open(config_location) as config_file:
        CONFIG.read(config_location)
except:
    CONFIG["DEFAULT"] = {
        "Fast Play Speed": False,
        "Infinite Playback": False,
        "Repeat Count": 1,
        "Recording Hotkey": 348,
        "Playback Hotkey": 349,
        "Always On Top": True,
        "Language": "en",
        "Recording Timer": 0,
        "Mouse Speed": 21,
    }
