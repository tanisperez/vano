#!/usr/bin/env python3
"""Moves the pointer through a virtual device, and clicks with "click" as the third argument.

usage: pointer.py X Y [click], X and Y in logical pixels of the desktop.
       pointer.py serve: keeps the device open and reads commands from the standard input, one per
       line, "glide X Y" (a smooth move) and "click", answering "ok" to each. For a recording.

`xdotool mousemove` does not move the pointer under XWayland on KDE Plasma with several monitors,
so this writes absolute positions to /dev/uinput, which needs python-evdev and write access to it.
"""
import math
import re
import subprocess
import sys
import time

from evdev import AbsInfo, UInput, ecodes as e

# KWin needs a moment to register a new device.
REGISTER_SECONDS = 0.7
AXIS_MAX = 32767
GLIDE_STEPS = 36
GLIDE_STEP_SECONDS = 0.016


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


def serve(device, move, press):
    """Answers the commands of the standard input until it closes."""
    x, y = None, None
    for line in sys.stdin:
        fields = line.split()
        if fields[0] == "click":
            press()
        else:
            to_x, to_y = float(fields[1]), float(fields[2])
            from_x, from_y = (to_x, to_y) if x is None else (x, y)
            for step in range(1, GLIDE_STEPS + 1):
                ease = 0.5 - math.cos(math.pi * step / GLIDE_STEPS) / 2
                move(from_x + (to_x - from_x) * ease, from_y + (to_y - from_y) * ease)
                time.sleep(GLIDE_STEP_SECONDS)
            x, y = to_x, to_y
        print("ok", flush=True)


def main():
    width, height = desktop_size()
    axis = AbsInfo(0, 0, AXIS_MAX, 0, 0, 0)
    device = UInput(
        {e.EV_KEY: [e.BTN_LEFT], e.EV_ABS: [(e.ABS_X, axis), (e.ABS_Y, axis)]},
        name="vano-screenshots-pointer",
    )
    time.sleep(REGISTER_SECONDS)

    def move(x, y):
        device.write(e.EV_ABS, e.ABS_X, int(x / width * AXIS_MAX))
        device.write(e.EV_ABS, e.ABS_Y, int(y / height * AXIS_MAX))
        device.syn()

    def press():
        device.write(e.EV_KEY, e.BTN_LEFT, 1)
        device.syn()
        time.sleep(0.1)
        device.write(e.EV_KEY, e.BTN_LEFT, 0)
        device.syn()
        time.sleep(0.2)

    if sys.argv[1:] == ["serve"]:
        print("ok", flush=True)
        serve(device, move, press)
    else:
        move(float(sys.argv[1]), float(sys.argv[2]))
        time.sleep(0.3)
        if sys.argv[3:] == ["click"]:
            press()
    device.close()


main()
