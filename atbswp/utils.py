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
# Handle the config file of the program

import configparser
import os
import platform


config = configparser.ConfigParser()


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
        config.write(config_file)

try:
    with open(config_location) as config_file:
        config.read(config_location)
except OSError:
    config['DEFAULT'] = {'Play Speed: Fast': False,
                         'Continuous Playback': False,
                         'Repeat Count': 1,
                         'Recording Hotkey': 'F9',
                         'Playback Hotkey': 'F10',
                         'Always On Top': False}
