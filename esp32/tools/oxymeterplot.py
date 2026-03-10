import queue
import re
import sys
import threading
from collections import deque

import matplotlib

matplotlib.use("QtAgg")
import matplotlib.animation as animation
import matplotlib.pyplot as plt

# Thread-safe queue to pass data from the terminal to the GUI
data_queue = queue.Queue()


def read_stdin():
    # Matches sequences like "Raw,123.4", "Clean,-0.5"
    pattern = re.compile(r"([a-zA-Z0-9_]+),([-+]?\d*\.\d+|\d+)")

    for line in sys.stdin:
        matches = pattern.findall(line)

        if matches:
            try:
                parsed_data = {name: float(val) for name, val in matches}
                data_queue.put(parsed_data)
            except ValueError:
                pass
        else:
            # If it's not CSV data, print it straight to the terminal!
            print(line, end="", flush=True)


# Start the background reader thread
t = threading.Thread(target=read_stdin, daemon=True)
t.start()

# --- Plot Setup ---
WINDOW_SIZE = 200
x_data = list(range(WINDOW_SIZE))

fig = plt.figure(figsize=(10, 6))
if fig.canvas.manager is not None:
    fig.canvas.manager.set_window_title("MAX30102 Live Telemetry")

# Track our dynamic signals
signal_names = []
data_buffers = {}
axes = {}
lines = {}


def update_plot(frame):
    updated = False
    new_signal_added = False

    # Empty the queue into our rolling buffers
    while not data_queue.empty():
        new_data = data_queue.get()

        for name, val in new_data.items():
            # If we detect a brand new signal name
            if name not in signal_names:
                signal_names.append(name)
                data_buffers[name] = deque([0.0] * WINDOW_SIZE, maxlen=WINDOW_SIZE)
                new_signal_added = True

            data_buffers[name].append(val)
            updated = True

    # If a new signal appeared, we need to rebuild the subplot stack
    if new_signal_added:
        num_plots = len(signal_names)

        for i, name in enumerate(signal_names):
            if name not in axes:
                # Add a new subplot at the bottom of the stack
                ax = fig.add_subplot(num_plots, 1, i + 1)
                (line,) = ax.plot(x_data, data_buffers[name], label=name)
                ax.legend(loc="upper right")

                axes[name] = ax
                lines[name] = line
            else:
                # Update the geometry of existing plots to make room
                axes[name].change_geometry(num_plots, 1, i + 1)

        # Redraw the layout so the new stack fits nicely
        fig.tight_layout()
        fig.canvas.draw_idle()

    # Update the data for all lines
    if updated:
        for name in signal_names:
            lines[name].set_ydata(data_buffers[name])
            axes[name].relim()
            axes[name].autoscale_view(True, True, True)

    return list(lines.values())


ani = animation.FuncAnimation(fig, update_plot, interval=20, cache_frame_data=False)

plt.show()
