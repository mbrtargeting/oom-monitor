# oom-monitor

Monitors the state of a system and prints system state if there is an OOM

## How To Test

(tested on Ubuntu 18.04 system)

First of all, compile and start the oom-monitor: `cargo run`.

(If you don't have the toolchain yet, you can get it here: <https://rustup.rs/>)

Open a python shell and run the following:

```python
listy = []
while True:
    listy.append(' ' * 51200000)

```

This should trigger the OOM killer! Please remember that excessive swapping might hurt your storage device.

Proceed to read the rather extensive log.
