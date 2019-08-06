extern crate chrono;
extern crate regex;

use sysinfo::{SystemExt, Pid, Process, ProcessExt};
use std::{thread, time};
use chrono::{DateTime, Utc};
use std::process::Command;
use std::str;
use std::collections::{HashMap, VecDeque};
use regex::Regex;

#[derive(Debug)]
struct SystemState {
    timestamp: DateTime<Utc>,
    total_memory: u64,
    used_memory: u64,
    total_swap: u64,
    used_swap: u64,
    processes: HashMap<Pid, Process>,
}

fn main() {
    let a_second = time::Duration::from_millis(1000);
    let mut system = sysinfo::System::new();
    let mut snapshots: VecDeque<SystemState> = VecDeque::new();
    let mut already_seen_ooms:HashMap<String, ()> = HashMap::new();
    let mut now_seen_ooms:HashMap<String, ()>;

    let output = get_dmesg_kill_lines().expect("Exiting! Could not fill hashmap with already seen OOMs");
    for line in output.lines() {
        already_seen_ooms.insert(line.to_owned(), ());
    }

    loop {
        system.refresh_all();

        snapshots.truncate(10);

        let current_system_state = SystemState {
            timestamp: Utc::now(),
            total_memory: system.get_total_memory(),
            used_memory: system.get_used_memory(),
            total_swap: system.get_total_swap(),
            used_swap: system.get_used_swap(),
            processes: system.get_process_list().to_owned(),
        };
        snapshots.push_front(current_system_state);

        thread::sleep(a_second);

        let maybe_kill_lines = get_dmesg_kill_lines();
        match maybe_kill_lines {
            Err(e) => println!("Problems with dmesg: {}", e),
            Ok(kill_lines) => {
                now_seen_ooms = HashMap::new();
                for line in kill_lines.lines() {
                    let is_new = !already_seen_ooms.contains_key(line);
                    now_seen_ooms.insert(line.to_owned(), ());
                    if is_new {
                        println!("Observed OOM kill. The time is {}", Utc::now().to_rfc3339());
                        let re = Regex::new(r"Killed process (\d*)").expect("Could not compile regex. Programmer error. Exiting.");
                        let killed_process_id = re.captures(line).expect(&format!("No captures in line \"{}\". Programmer error. Exiting.", line))
                            .get(1).expect("Could not match PID. Programmer error. Exiting.")
                            .as_str().parse::<i32>().expect("Process ID could not be mapped to int. Programmer error. Exiting.");
                        let maybe_last_snapshot = get_snapshot_with_killed_process(&snapshots, killed_process_id);
                        match maybe_last_snapshot {
                            None => println!("No snapshot with killed process in queue. That's not supposed to happen."),
                            Some(snapshot) => {
                                println!("Found snapshot of system state with killed process. Snapshot taken at {}", snapshot.timestamp.to_rfc3339());
                                println!("Memory: Used {} out of {}", snapshot.used_memory, snapshot.total_memory);
                                println!("Swap: Used {} out of {}", snapshot.used_swap, snapshot.total_swap);
                                let maybe_killed_process = snapshot.processes.get(&killed_process_id);
                                match maybe_killed_process {
                                    None => println!("get_snapshot_with_killed_process malfunctioned. Should never happen"),
                                    Some(killed_process) => println!("The following process was killed: {:?}", killed_process)
                                }
                                let mut processes:Vec<Process> = snapshot.processes.iter().map(|(_, process)| process.clone()).collect();
                                processes.sort_by_key(|process| process.memory());
                                println!("Processes, sorted by memory usage:");
                                for process in processes {
                                    println!("PID: {}\tName: {}\t Memory:{}\t CMD: {:?}", process.pid(), process.name(), process.memory(), process.cmd());
                                }
                            }
                        }
                        println!("\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#");
                    }
                }
                already_seen_ooms = now_seen_ooms;
            },
        }
    }

    
}

fn get_snapshot_with_killed_process(snapshots: &VecDeque<SystemState>, killed_process_id: i32) -> Option<&SystemState> {
    for snapshot in snapshots {
        if snapshot.processes.contains_key(&killed_process_id) {
            return Some(&snapshot)
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

fn get_dmesg_kill_lines() -> std::result::Result<String, String> {
    let maybe_output = Command::new("dmesg").arg("--time-format").arg("iso").arg("--decode").arg("--nopager").output();
    match maybe_output {
        Err(e) => Err(format!("Could not read from dmesg: {}", e)),
        Ok(output) => {
            if !output.status.success() {
                let stderr = to_utf8_or_raw(&output.stderr);
                Err(format!("dmesg failed with error: {}", stderr))
            } else {
                match str::from_utf8(&output.stdout) {
                    Err(_e) => Err(format!("Could not deserialize to unicode: {:?}", output.stdout)),
                    Ok(unicode) => {
                        Ok(unicode.lines().filter(|line| line.contains("Killed process")).collect())
                    }
                }
            }
        }
    }
}