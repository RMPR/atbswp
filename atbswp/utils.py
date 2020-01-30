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
import configparser



import configparser as ConfigParser

def load_parameters(path, param, scope="atbswp"):
    """
    Usage:
        load_parameters("./example_config.txt", "EXAMPLE")
    
    Params:
        path :  the path of the configuration file
        param : the key of the parameter you want to get the value
        scope :  The scope of parameters
    """
    # Configs parameters
    configParser = ConfigParser.RawConfigParser()   
    configFilePath = path
    configParser.read(configFilePath)

    # Filling parameters
    return configParser.get(scope, param)