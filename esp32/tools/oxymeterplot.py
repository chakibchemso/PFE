"""
Live scrolling plotter for ESP32 telemetry.

Reads compact text protocol from stdin:
  @R:<id>:<name>:<r>,<g>,<b>  - register channel
  @D:<id>:<value>              - data point
  @C                           - clear all

Renders fixed-size scrolling graphs in a grid layout using matplotlib.
Dynamically adds subplots as new channels are registered.
"""

import queue
import sys
import threading
from collections import deque
from typing import Iterable

import matplotlib

matplotlib.use("QtAgg")
import matplotlib.animation as animation
import matplotlib.figure
import matplotlib.pyplot as plt

# Thread-safe queue to pass data from stdin to the GUI
data_queue: queue.Queue[str] = queue.Queue()

# ─── Configuration ───────────────────────────────────────────────────────────

BUFFER_SIZE = 200  # samples visible in the scroll window
FIG_BG = (0.07, 0.07, 0.095)  # dark background
AXES_BG = (0.09, 0.09, 0.125)
GRID_COLOR = (0.18, 0.18, 0.22)
TEXT_COLOR = (0.78, 0.78, 0.82)
UPDATE_INTERVAL = 33  # ms (~30 fps, balances smoothness and CPU)


# ─── Data Layer ──────────────────────────────────────────────────────────────


def stdin_reader():
    """Read lines from stdin and push plot frames to the queue."""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            # Skip blank lines
            continue
        if line.startswith("@"):
            data_queue.put(line)
        else:
            sys.stdout.write(line + "\n")
            sys.stdout.flush()


# Channel state
channels: dict[int, dict] = {}  # id -> {name, color, buffer}
channel_order: list[int] = []  # insertion order
fig: matplotlib.figure.Figure | None = None
axes_list: list = []  # [(ax, line, ch_id), ...]
waiting_text = None  # "Waiting for data..." text handle


def parse_frame(line: str):
    """Parse a @R/@D/@C frame and update channel state."""
    if line == "@C":
        channels.clear()
        channel_order.clear()
        return

    parts = line.split(":")
    if len(parts) < 3:
        return

    frame_type = parts[0]

    if frame_type == "@R" and len(parts) >= 4:
        # @R:<id>:<name>:<r>,<g>,<b>
        try:
            ch_id = int(parts[1])
            name = parts[2]
            rgb = parts[3].split(",")
            color = (int(rgb[0]) / 255, int(rgb[1]) / 255, int(rgb[2]) / 255)
        except (ValueError, IndexError):
            return

        if ch_id not in channels:
            channels[ch_id] = {
                "name": name,
                "color": color,
                "buffer": deque([0.0] * BUFFER_SIZE, maxlen=BUFFER_SIZE),
            }
            channel_order.append(ch_id)

    elif frame_type == "@D" and len(parts) >= 3:
        # @D:<id>:<value>
        try:
            ch_id = int(parts[1])
            value = float(parts[2])
        except (ValueError, IndexError):
            return

        if ch_id in channels:
            channels[ch_id]["buffer"].append(value)


def rebuild_layout():
    """Rebuild the entire subplot layout to accommodate new channels."""
    global axes_list, waiting_text, fig

    num_plots = len(channel_order)
    if num_plots == 0:
        return

    assert fig is not None

    # Clear the figure completely
    fig.clear()
    fig.set_facecolor(FIG_BG)
    axes_list = []
    x_data = list(range(BUFFER_SIZE))

    for i, ch_id in enumerate(channel_order):
        ch = channels[ch_id]
        ax = fig.add_subplot(num_plots, 1, i + 1)
        ax.set_facecolor(AXES_BG)
        ax.set_title(
            ch["name"], color=ch["color"], fontsize=10, loc="left", weight="bold"
        )

        (line,) = ax.plot(x_data, list(ch["buffer"]), color=ch["color"], linewidth=1.2)
        ax.grid(True, color=GRID_COLOR, linewidth=0.5, alpha=0.5)
        ax.tick_params(colors=TEXT_COLOR)
        for spine in ax.spines.values():
            spine.set_color(GRID_COLOR)

        axes_list.append((ax, line, ch_id))

    fig.tight_layout(pad=1.5)
    waiting_text = None  # No longer showing waiting message


def update_plot(_frame) -> Iterable:
    """Update plot data each frame."""
    global fig

    # Drain the queue
    new_channel_added = False
    while not data_queue.empty():
        try:
            line = data_queue.get_nowait()
            # Check if this is a new channel registration before parsing
            if line.startswith("@R:"):
                parts = line.split(":")
                if len(parts) >= 4:
                    try:
                        ch_id = int(parts[1])
                        if ch_id not in channels:
                            new_channel_added = True
                    except ValueError:
                        pass
            parse_frame(line)
        except queue.Empty:
            break

    # If new channels were added, rebuild the layout
    if new_channel_added:
        rebuild_layout()
        return []

    if not axes_list:
        return []

    result = []
    for ax, line, ch_id in axes_list:
        if ch_id in channels:
            ch = channels[ch_id]
            buf = ch["buffer"]

            if len(buf) > 1:
                line.set_ydata(list(buf))

                # Auto-scale Y
                data_min = min(buf)
                data_max = max(buf)
                margin = max((data_max - data_min) * 0.1, 0.5)
                ax.set_ylim(data_min - margin, data_max + margin)

                # Update title with current value
                current = buf[-1]
                ax.set_title(
                    f"{ch['name']}  =  {current:.1f}",
                    color=ch["color"],
                    fontsize=10,
                    loc="left",
                    weight="bold",
                )

                result.append(line)

    return result


# ─── Entry Point ─────────────────────────────────────────────────────────────

if __name__ == "__main__":
    # Start stdin reader in background
    t = threading.Thread(target=stdin_reader, daemon=True)
    t.start()

    # Initialize empty figure with "waiting" message
    fig = plt.figure(figsize=(10, 4), facecolor=FIG_BG)
    assert fig.canvas.manager is not None
    fig.canvas.manager.set_window_title("ESP32 Live Plotter")

    ax = fig.add_subplot(1, 1, 1)
    ax.set_facecolor(AXES_BG)
    waiting_text = ax.text(
        0.5,
        0.5,
        "Waiting for data...",
        ha="center",
        va="center",
        color=TEXT_COLOR,
        fontsize=14,
        transform=ax.transAxes,
    )
    ax.tick_params(colors=TEXT_COLOR)
    for spine in ax.spines.values():
        spine.set_color(GRID_COLOR)

    fig.tight_layout()

    ani = animation.FuncAnimation(
        fig, update_plot, interval=UPDATE_INTERVAL, cache_frame_data=False
    )

    plt.show()
