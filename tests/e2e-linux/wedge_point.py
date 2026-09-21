#!/usr/bin/env python3
"""Prints the screen coordinates of a WedgeMenu option's clickable midpoint.

Mirrors src/lib/wedgeGeometry.ts's polarToCartesian/wedgeAngles plus
WedgeMenu.tsx's own geometry constants (SIZE=320, CENTER=160,
OUTER_RADIUS=150, INNER_RADIUS=40, LABEL_RADIUS=95). The overlay window is
always centered on the cursor position at trigger time (see
overlay_position in crates/satsuma-core/src/overlay_geometry.rs), and the
wedge SVG renders at its native 320x320 size with no CSS scaling inside the
480x480 overlay window (OVERLAY_SIZE in src-tauri/src/lib.rs, matching
src-tauri/tauri.conf.json), centered there by flexbox — so the SVG's own
center always lands exactly on whatever cursor position the harness itself
chose before launching the app. That's why this only needs the cursor
position plus the wedge index/count, never an actual window-geometry query.

If SIZE/CENTER/OUTER_RADIUS/INNER_RADIUS/LABEL_RADIUS ever change in
WedgeMenu.tsx, update LABEL_RADIUS below to match.

Usage: wedge_point.py <cursor_x> <cursor_y> <option_index> <option_count>
Prints: "<x> <y>" (integers, ready for `xdotool mousemove`)
"""
import math
import sys

LABEL_RADIUS = 95  # (OUTER_RADIUS 150 + INNER_RADIUS 40) / 2


def wedge_point(cursor_x: int, cursor_y: int, index: int, count: int) -> tuple[int, int]:
    if count <= 0:
        raise ValueError("count must be positive")
    if not 0 <= index < count:
        raise ValueError(f"index {index} out of range for count {count}")
    # wedgeAngles splits a full circle into `count` equal wedges starting at
    # the top and going clockwise; this is the midpoint of wedge `index`.
    angle_mid_deg = (index + 0.5) * 360 / count
    # polarToCartesian measures its angle clockwise from the top (subtracts
    # 90 degrees before converting to standard math radians).
    angle_rad = math.radians(angle_mid_deg - 90)
    x = cursor_x + LABEL_RADIUS * math.cos(angle_rad)
    y = cursor_y + LABEL_RADIUS * math.sin(angle_rad)
    return round(x), round(y)


def main() -> None:
    if len(sys.argv) != 5:
        print(f"usage: {sys.argv[0]} <cursor_x> <cursor_y> <option_index> <option_count>", file=sys.stderr)
        sys.exit(1)
    cursor_x, cursor_y, index, count = (int(arg) for arg in sys.argv[1:5])
    x, y = wedge_point(cursor_x, cursor_y, index, count)
    print(f"{x} {y}")


if __name__ == "__main__":
    main()
