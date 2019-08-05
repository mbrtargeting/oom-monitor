# oom-monitor

Monitors the state of a system and prints system state if there is an OOM

## How To Test

(tested on Ubuntu 18.04 system)

First of all, compile and start the oom-monitor: `cargo run`

You can trigger a sweep by the OOM killer by running the following as root:

```bash
sudo echo "1" > /proc/sys/kernel/sysrq && sudo echo f > /proc/sysrq-trigger && dmesg -x -T && sudo echo "176" > /proc/sys/kernel/sysrq
```

However it won't have much effect unless you have a real memory hog somewhere. Have a terminal with the above line ready (as root), at the same time open a python shell and run the following:

```python
listy = []
while True:
    listy.append(' ' * 512000000)

```

Now run the OOM killer! Please remember that excessive swapping might hurt your storage device.

Proceed to read the rather extensive log.
