
#!/bin/env python3
# Created by atbswp (https://github.com/rmpr/atbswp)
# on 29 Apr 2021
import pyautogui
import time
pyautogui.FAILSAFE = False


def exec_captured(stop_flag : list):
    for move in movements:
        if stop_flag[0]: break
        eval(move)


movements= [

"pyautogui.moveTo(514, 502)",
"pyautogui.moveTo(463, 524)",
"pyautogui.moveTo(488, 383)",
"pyautogui.moveTo(509, 373)",
"pyautogui.moveTo(533, 374)",
"pyautogui.moveTo(552, 397)",
"pyautogui.moveTo(565, 422)",
"pyautogui.moveTo(575, 446)",
"pyautogui.moveTo(585, 468)",
"pyautogui.moveTo(609, 475)",
"pyautogui.moveTo(634, 451)",
"pyautogui.moveTo(657, 415)",
"pyautogui.moveTo(679, 348)",
"pyautogui.moveTo(573, 372)",
"pyautogui.moveTo(569, 394)",
"time.sleep(1)",
"pyautogui.moveTo(590, 285)",
"pyautogui.moveTo(611, 236)",
"pyautogui.moveTo(632, 213)",
"pyautogui.moveTo(653, 193)",
    ]

