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

import wx


class MainDialog(wx.Dialog):
    def __init__(self, *args, **kwds):
        kwds["style"] = kwds.get("style", 0) | wx.DEFAULT_DIALOG_STYLE
        wx.Dialog.__init__(self, *args, **kwds)
        self.folder_open = wx.BitmapButton(self,
                                           wx.ID_ANY,
                                           wx.Bitmap("./img/file-upload.png",
                                                     wx.BITMAP_TYPE_ANY))
        self.save = wx.BitmapButton(self,
                                    wx.ID_ANY,
                                    wx.Bitmap("./img/save.png",
                                              wx.BITMAP_TYPE_ANY))
        self.record = wx.BitmapButton(self,
                                      wx.ID_ANY,
                                      wx.Bitmap("./img/video.png",
                                                wx.BITMAP_TYPE_ANY))
        self.play = wx.BitmapButton(self,
                                    wx.ID_ANY,
                                    wx.Bitmap("./img/play-circle.png",
                                              wx.BITMAP_TYPE_ANY))
        self.compile = wx.BitmapButton(self,
                                       wx.ID_ANY,
                                       wx.Bitmap("./img/download.png",
                                                 wx.BITMAP_TYPE_ANY))
        self.settings = wx.BitmapButton(self,
                                        wx.ID_ANY,
                                        wx.Bitmap("./img/cog.png",
                                                  wx.BITMAP_TYPE_ANY))

        self.__set_properties()
        self.__do_layout()

    def __set_properties(self):
        self.folder_open.SetSize(self.folder_open.GetBestSize())
        self.save.SetSize(self.save.GetBestSize())
        self.record.SetSize(self.record.GetBestSize())
        self.play.SetSize(self.play.GetBestSize())
        self.compile.SetSize(self.compile.GetBestSize())
        self.settings.SetSize(self.settings.GetBestSize())

    def __do_layout(self):
        main_sizer = wx.BoxSizer(wx.HORIZONTAL)
        main_sizer.Add(self.folder_open, 0, 0, 0)
        main_sizer.Add(self.save, 0, 0, 0)
        main_sizer.Add(self.record, 0, 0, 0)
        main_sizer.Add(self.play, 0, 0, 0)
        main_sizer.Add(self.compile, 0, 0, 0)
        main_sizer.Add(self.settings, 0, 0, 0)
        self.SetSizer(main_sizer)
        main_sizer.Fit(self)
        self.Layout()


class Atbswp(wx.App):
    def OnInit(self):
        self.main = MainDialog(None, wx.ID_ANY, "atbswp")
        self.SetTopWindow(self.main)
        self.main.ShowModal()
        self.main.Destroy()
        return True


if __name__ == "__main__":
    app = Atbswp(0)
    app.MainLoop()
