#!/usr/bin/env python3
"""Moves the pointer through a virtual device, and clicks with "click" as the third argument.

usage: pointer.py X Y [click], X and Y in logical pixels of the desktop.

`xdotool mousemove` does not move the pointer under XWayland on KDE Plasma with several monitors,
so this writes absolute positions to /dev/uinput, which needs python-evdev and write access to it.
"""
import re
import subprocess
import sys
import time

from evdev import AbsInfo, UInput, ecodes as e

# KWin needs a moment to register a new device.
REGISTER_SECONDS = 0.7
AXIS_MAX = 32767


def desktop_size():
    """The bounding box of every output, in logical pixels."""
    text = subprocess.run(
        ["kscreen-doctor", "-o"], capture_output=True, text=True, check=True
    ).stdout
    text = re.sub(r"\x1b\[[0-9;]*m", "", text)
    outputs = re.findall(r"Geometry: (\d+),(\d+) (\d+)x(\d+)", text)
    return (
        max(int(x) + int(w) for x, _, w, _ in outputs),
        max(int(y) + int(h) for _, y, _, h in outputs),
    )


def main():
    x, y = float(sys.argv[1]), float(sys.argv[2])
    click = sys.argv[3:] == ["click"]
    width, height = desktop_size()
    axis = AbsInfo(0, 0, AXIS_MAX, 0, 0, 0)
    device = UInput(
        {e.EV_KEY: [e.BTN_LEFT], e.EV_ABS: [(e.ABS_X, axis), (e.ABS_Y, axis)]},
        name="vano-screenshots-pointer",
    )
    time.sleep(REGISTER_SECONDS)
    device.write(e.EV_ABS, e.ABS_X, int(x / width * AXIS_MAX))
    device.write(e.EV_ABS, e.ABS_Y, int(y / height * AXIS_MAX))
    device.syn()
    time.sleep(0.3)
    if click:
        device.write(e.EV_KEY, e.BTN_LEFT, 1)
        device.syn()
        time.sleep(0.1)
        device.write(e.EV_KEY, e.BTN_LEFT, 0)
        device.syn()
        time.sleep(0.2)
    device.close()


main()
