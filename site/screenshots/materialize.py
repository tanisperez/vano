"""Turns the manifest of demo_home into real files under a folder, each file allocated with
fallocate at a fraction of its size, so the app can scan the made-up home live.

Usage: python3 materialize.py <manifest> <home> <divisor>
"""

import os
import sys


def main():
    manifest, home, divisor = sys.argv[1], sys.argv[2], int(sys.argv[3])
    dirs = []
    with open(manifest, encoding="utf-8") as lines:
        for line in lines:
            fields = line.rstrip("\n").split("\t")
            path = os.path.join(home, fields[1])
            if fields[0] == "D":
                os.makedirs(path, exist_ok=True)
                dirs.append(path)
                continue
            size, mtime = int(fields[2]) // divisor, int(fields[3])
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
            try:
                if size > 0:
                    os.posix_fallocate(fd, 0, size)
            finally:
                os.close(fd)
            os.utime(path, (mtime, mtime))
    # Children first, so creating a file never touches a folder dated already.
    for path in reversed(dirs):
        newest = max((e.stat().st_mtime for e in os.scandir(path)), default=None)
        if newest is not None:
            os.utime(path, (newest, newest))


main()
