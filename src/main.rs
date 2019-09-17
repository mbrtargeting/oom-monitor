extern crate chrono;
extern crate regex;

#[macro_use]
extern crate log;
extern crate env_logger;

use chrono::{DateTime, Duration, Utc};
use env_logger::Env;
use regex::Regex;
use std::collections::{HashMap, VecDeque};
use std::env;
use std::io::Write;
use std::process;
use std::process::Command;
use std::str;
use std::{thread, time};
use sysinfo::{Pid, Process, ProcessExt, RefreshKind, SystemExt};

#[derive(Debug)]
struct SystemState {
    timestamp: DateTime<Utc>,
    total_memory: u64,
    used_memory: u64,
    total_swap: u64,
    used_swap: u64,
    processes: HashMap<Pid, Process>,
}

impl SystemState {
    fn new(maybe_system: Option<&sysinfo::System>) -> SystemState {
        fn newState(system: &sysinfo::System) -> SystemState {
            return SystemState {
                timestamp: Utc::now(),
                total_memory: system.get_total_memory(),
                used_memory: system.get_used_memory(),
                total_swap: system.get_total_swap(),
                used_swap: system.get_used_swap(),
                processes: system.get_process_list().to_owned(),
            };
        }
        match maybe_system {
            None => {
                let system = sysinfo::System::new_with_specifics(
                    RefreshKind::new().with_system().with_processes(),
                );
                return newState(&system);
            }
            Some(system) => {
                return newState(system);
            }
        };
    }
}

struct MaxMemData {
    max_mem_snapshot: SystemState,
    have_recently_printed_max_mem_usage: bool,
}

struct OomData {
    snapshots: VecDeque<SystemState>,
    already_seen_ooms: HashMap<String, ()>,
}

fn main() {
    let mut builder = env_logger::from_env(Env::default().default_filter_or("info"));
    if env::var("RUST_LOG_NO_FORMAT") == Ok("true".to_owned()) {
        &builder.format(|buf, record| writeln!(buf, "{}: {}", record.level(), record.args()));
    }
    builder.init();

    let a_second = time::Duration::from_millis(1000);
    let mut already_seen_ooms: HashMap<String, ()> = HashMap::new();

    let mut max_mem_data = MaxMemData {
        max_mem_snapshot: SystemState::new(None),
        have_recently_printed_max_mem_usage: false,
    };

    match get_dmesg_kill_lines() {
        Err(e) => {
            error!("Could not fill hashmap with already seen OOMs: {}", e);
            error!("Fatal flaw in program or environment. Exiting.");
            process::exit(1);
        }
        Ok(output) => {
            for line in output {
                already_seen_ooms.insert(line.to_owned(), ());
            }
        }
    }

    let mut oom_data = OomData {
        snapshots: VecDeque::new(),
        already_seen_ooms: already_seen_ooms,
    };

    let mut system = sysinfo::System::new();
    let mut countdown_to_system_reinstantiation = 100;

    loop {
        // sysinfo::System leaks memory, but is expensive to instantiate
        countdown_to_system_reinstantiation = countdown_to_system_reinstantiation - 1;
        if countdown_to_system_reinstantiation == 0 {
            countdown_to_system_reinstantiation = 100;
            system = sysinfo::System::new();
        }

        system.refresh_system();
        system.refresh_processes();

        max_mem_data = handle_max_mem_statistics(max_mem_data, &system);
        oom_data = handle_ooms(oom_data, &system);

        thread::sleep(a_second);
    }
}

fn handle_ooms(oom_data: OomData, system: &sysinfo::System) -> OomData {
    let mut snapshots = oom_data.snapshots;

    snapshots.push_front(SystemState::new(Some(system)));
    snapshots.truncate(10);

    let maybe_kill_lines = get_dmesg_kill_lines();
    match maybe_kill_lines {
        Err(e) => {
            if e.contains("Out of memory") {
                warn!("System state means dmesg has problems: {}", e);
            } else {
                error!("Problems with dmesg: {}", e);
            }
            return OomData {
                snapshots: snapshots,
                already_seen_ooms: oom_data.already_seen_ooms,
            };
        }
        Ok(kill_lines) => {
            let mut now_seen_ooms = HashMap::new();
            for line in kill_lines {
                let is_new = !oom_data.already_seen_ooms.contains_key(&line);
                now_seen_ooms.insert(line.to_owned(), ());
                if is_new {
                    info!(
                        "Observed OOM kill. The dmesg line looks like this: \"{}\"",
                        line
                    );
                    match extract_pid_from_kill_line(&line) {
                        Err(e) => {
                            error!("Failed to extract pid from kill line: {}", e);
                            error!("Fatal flaw in program. Exiting.");
                            process::exit(1);
                        }
                        Ok(killed_process_id) => {
                            let maybe_last_snapshot =
                                get_snapshot_with_killed_process(&snapshots, killed_process_id);
                            match maybe_last_snapshot {
                                None => match snapshots.front() {
                                    None => error!(
                                        "No snapshots in queue, so we have nothing to print."
                                    ),
                                    Some(snapshot) => {
                                        error!("No snapshot with killed process in queue. For debugging purposes, we'll print out the last snapshot");
                                        print_processes_by_memory(snapshot)
                                    }
                                },
                                Some(snapshot) => {
                                    info!("Found snapshot of system state with killed process. Snapshot taken at {}", snapshot.timestamp.to_rfc3339());
                                    info!(
                                        "Memory: Used {} out of {}, or {}%",
                                        snapshot.used_memory,
                                        snapshot.total_memory,
                                        memory_percentage(
                                            snapshot.used_memory,
                                            snapshot.total_memory
                                        )
                                    );
                                    info!(
                                        "Swap: Used {} out of {}, or {}%",
                                        snapshot.used_swap,
                                        snapshot.total_swap,
                                        memory_percentage(snapshot.used_swap, snapshot.total_swap)
                                    );
                                    let maybe_killed_process =
                                        snapshot.processes.get(&killed_process_id);
                                    match maybe_killed_process {
                                            None => error!("get_snapshot_with_killed_process malfunctioned. Should never happen"),
                                            Some(killed_process) => info!("The following process was killed: {}", process_to_long_string(killed_process, &snapshot))
                                        }
                                    print_processes_by_memory(snapshot)
                                }
                            }
                            info!("\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#");
                        }
                    }
                }
            }
            return OomData {
                snapshots: snapshots,
                already_seen_ooms: now_seen_ooms,
            };
        }
    }
}

fn handle_max_mem_statistics(max_mem_data: MaxMemData, system: &sysinfo::System) -> MaxMemData {
    let now = Utc::now();
    let next_midnight = (now + Duration::days(1)).date().and_hms(0, 0, 0);
    let previous_midnight = now.date().and_hms(0, 0, 0);
    let midday = now.date().and_hms(12, 0, 0);
    let ready_to_print = !max_mem_data.have_recently_printed_max_mem_usage;

    let current_snapshot = SystemState::new(Some(system));

    if ready_to_print
        && ((next_midnight - now).num_seconds().abs() < 10
            || (previous_midnight - now).num_seconds().abs() < 10)
    {
        let system_total_memory = system.get_total_memory();
        info!(
            "Max seen memory usage throughout the day: {}kB. That's {}%",
            max_mem_data.max_mem_snapshot.used_memory,
            memory_percentage(
                max_mem_data.max_mem_snapshot.used_memory,
                system_total_memory
            )
        );
        info!("Here is the system at that time:");
        print_processes_by_memory(&max_mem_data.max_mem_snapshot);
        return MaxMemData {
            max_mem_snapshot: SystemState::new(Some(system)),
            have_recently_printed_max_mem_usage: true,
        };
    }
    if (midday - now).num_seconds().abs() < 10 {
        return MaxMemData {
            max_mem_snapshot: systemstate_with_highest_mem_usage(
                max_mem_data.max_mem_snapshot,
                current_snapshot,
            ),
            have_recently_printed_max_mem_usage: false,
        };
    }
    return MaxMemData {
        max_mem_snapshot: systemstate_with_highest_mem_usage(
            max_mem_data.max_mem_snapshot,
            current_snapshot,
        ),
        have_recently_printed_max_mem_usage: max_mem_data.have_recently_printed_max_mem_usage,
    };
}

fn systemstate_with_highest_mem_usage(a: SystemState, b: SystemState) -> SystemState {
    if a.used_memory > b.used_memory {
        return a;
    } else {
        return b;
    }
}

fn extract_pid_from_kill_line(line: &str) -> Result<i32, String> {
    match Regex::new(r"Killed process (\d*)") {
        Err(e) => Err(format!("Could not compile regex: {}", e)),
        Ok(re) => match re.captures(&line) {
            None => Err(format!(
                "No captures in line \"{}\" even though it was registered as a kill line.",
                line
            )),
            Some(cap) => match cap.get(1) {
                None => Err(format!("Could not match PID.")),
                Some(pidstring) => match pidstring.as_str().parse::<i32>() {
                    Err(e) => Err(format!("Process ID could not be mapped to int: {}", e)),
                    Ok(pid) => Ok(pid),
                },
            },
        },
    }
}

fn get_snapshot_with_killed_process(
    snapshots: &VecDeque<SystemState>,
    killed_process_id: i32,
) -> Option<&SystemState> {
    for snapshot in snapshots {
        if snapshot.processes.contains_key(&killed_process_id) {
            return Some(&snapshot);
        }
    }
    None
}

fn to_utf8_or_raw(presumably_unicode: &Vec<u8>) -> String {
    match str::from_utf8(presumably_unicode) {
        Err(_e) => format!("Could not deserialize to unicode: {:?}", presumably_unicode),
        Ok(unicode) => unicode.to_string(),
    }
}

fn get_dmesg_kill_lines() -> std::result::Result<Vec<String>, String> {
    let maybe_output = Command::new("dmesg")
        .arg("--time-format")
        .arg("iso")
        .arg("--decode")
        .arg("--nopager")
        .output();
    match maybe_output {
        Err(e) => Err(format!("Could not read from dmesg: {}", e)),
        Ok(output) => {
            if !output.status.success() {
                let stderr = to_utf8_or_raw(&output.stderr);
                Err(format!("dmesg failed with error: {}", stderr))
            } else {
                match str::from_utf8(&output.stdout) {
                    Err(_e) => Err(format!(
                        "Could not deserialize to unicode: {:?}",
                        output.stdout
                    )),
                    Ok(unicode) => Ok(unicode
                        .lines()
                        .filter(|line| line.contains("Killed process"))
                        .map(|x| x.to_owned())
                        .collect()),
                }
            }
        }
    }
}

fn process_to_long_string(process: &Process, snapshot: &SystemState) -> String {
    format!(
        "PID: {}
    Name: {}
    Memory: {}kB or {}%
    CPU: {}%
    Parent: {}
    CMD: {:?}
    Environment: {}
    Status: {}
    Start time: {}s
    CWD: {:?}
    Root: {:?}
    Executable: {:?}",
        process.pid(),
        process.name(),
        process.memory(),
        memory_percentage(process.memory(), snapshot.total_memory),
        process.cpu_usage(),
        parent_to_string(process.parent()),
        process.cmd(),
        stringlist_to_string(process.environ()),
        process.status(),
        process.start_time(),
        process.cwd(),
        process.root(),
        process.exe()
    )
    .to_owned()
}

fn stringlist_to_string(list: &[String]) -> String {
    let mut accumulator = "[".to_owned() + list.first().unwrap_or(&"".to_owned());
    for i in 1..list.len() {
        accumulator = accumulator + ", " + &list[i];
    }
    accumulator + "]"
}

fn memory_percentage(used: u64, total: u64) -> f32 {
    (100.0 * used as f64 / total as f64) as f32
}

fn parent_to_string(parent: Option<i32>) -> String {
    match parent {
        Some(pid) => pid.to_string(),
        None => "None".to_owned(),
    }
}

fn get_user_by_uid(uid: u32) -> String {
    match users::get_user_by_uid(uid) {
        None => "None".to_owned(),
        Some(user) => user.name().to_str().unwrap_or("").to_owned(),
    }
}

fn print_processes_by_memory(snapshot: &SystemState) {
    let mut processes: Vec<Process> = snapshot
        .processes
        .iter()
        .map(|(_, process)| process.clone())
        .collect();
    processes.sort_by_key(|process| process.memory());
    info!("Processes, sorted by memory usage:");
    info!(
        "{:17} {:7} {:7} {:30} {:9}kB {:7.7}% {:7.7}% {}",
        "User", "PID", "PPID", "Name (truncated)", "Mem ", "Mem ", "CPU ", "CMD"
    );
    for process in processes {
        info!(
            "{:17} {:7} {:7} {:30.30} {:9}kB {:5.5}% {:5.5}% {:?}",
            get_user_by_uid(process.uid),
            process.pid(),
            parent_to_string(process.parent()),
            process.name(),
            process.memory(),
            memory_percentage(process.memory(), snapshot.total_memory),
            process.cpu_usage(),
            process.cmd()
        );
    }
}
