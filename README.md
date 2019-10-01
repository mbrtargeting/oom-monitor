# oom-monitor

Helps you squeeze more performance out of your cluster and diagnose memory problems.

Two main features:

- If you've over-provisioned, it helps diagnose OOM problems by providing a snapshot of system activity right before the OOM kill happened.
- If you've under-provisioned, it helps you tell how much more you can give by providing a snapshot of system activity at the time of daily peak memory usage.

It achieves this by having a buffer of recorded system states updated periodically, and by polling dmesg. Whenever the Linux kernel records an OOM kill, the oom-monitor logs out the most recent system state snapshot it has where the killed process was still running. It also stores a snapshot of the system state at peak daily memory usage, which is printed and reset to zero at midnight.

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

## Building and packaging

(tested on Ubuntu 18.04 system)

Only support creating deb packages for now, although the raw binary should work on all (amd64?) Linux systems. If you want to just build a raw binary, you can do so using `cargo build --release`.

The `debianize` scripts require `fpm`. Install by following the instructions here: <https://fpm.readthedocs.io/en/latest/installing.html>

Then run `./debianize-gnu` or `./debianize-musl` while in the root folder of the project, depending on which clib you want to use.
