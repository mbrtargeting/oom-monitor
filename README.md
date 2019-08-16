# oom-monitor

Helps diagnose OOM problems by providing a snapshot of system activity right before the OOM kill happened.

It achieves this by having a buffer of recorded system states updated periodically, and by polling dmesg. Whenever the Linux kernel records an OOM kill, the oom-monitor logs out the most recent system state snapshot it has where the killed process was still running.

## How To Test

(tested on Ubuntu 18.04 system)

First of all, compile and start the oom-monitor: `cargo run --release`.

(If you don't have the toolchain yet, you can get it here: <https://rustup.rs/>)

Open a python shell and run the following:

```python
listy = []
while True:
    listy.append(' ' * 51200000)

```

This should trigger the OOM killer! Please remember that excessive swapping might hurt your storage device.

Sometimes it can be helpful to trigger an OOM sweep manually. You can do this by running the following as root:

```bash
sudo echo "1" > /proc/sys/kernel/sysrq && sudo echo f > /proc/sysrq-trigger && dmesg -x -T && sudo echo "176" > /proc/sys/kernel/sysrq
```

(replace 176 with whatever value is present in `/proc/sys/kernel/sysrq` by default on your computer)
