extern crate chrono;
extern crate regex;

use sysinfo::{SystemExt, Pid, Process};
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

        let maybe_output = Command::new("dmesg").arg("--time-format").arg("iso").arg("--decode").arg("--nopager").output();
        //dmesg --human -T -x
        match maybe_output {
            Err(e) => println!("Could not read from dmesg: {}", e),
            Ok(output) => {
                if !output.status.success() {
                    let stderr = to_utf8_or_raw(&output.stderr);
                    println!("dmesg failed with error: {}", stderr);
                } else {
                    match str::from_utf8(&output.stdout) {
                        Err(_e) => println!("Could not deserialize to unicode: {:?}", output.stdout),
                        Ok(unicode) => {
                            for line in unicode.lines() {
                                if line.contains("Killed process") {
                                    let re = Regex::new(r"Killed process (\d*)").expect("Could not compile regex. Programmer error. Exiting.");
                                    let killed_process_id = re.captures(line).expect(&format!("No captures in line \"{}\". Programmer error. Exiting.", line))
                                        .get(1).expect("Could not match PID. Programmer error. Exiting.")
                                        .as_str().parse::<i32>().expect("Process ID could not be mapped to int. Programmer error. Exiting.");
                                    let is_new = dmesg_line_newer_than(line, &snapshots.back().unwrap().timestamp);
                                    match is_new {
                                        Err(e) => println!("{}", e),
                                        Ok(false) => continue,
                                        Ok(true) => {
                                            let maybe_last_snapshot = snapshots.front();
                                            match maybe_last_snapshot {
                                                None => println!("No snapshot in queue. That's not supposed to happen."),
                                                Some(state) => {
                                                    let maybe_killed_process = state.processes.get(&killed_process_id);
                                                    match maybe_killed_process {
                                                        None => println!("Could not find the killed process in last system snapshot. Probably it all happened too fast"),
                                                        Some(killed_process) => println!("The following process was killed: {:?}", killed_process)
                                                    }
                                                }
                                            }
                                            for snapshot in &snapshots {
                                                println!("{:?}", snapshot);
                                                println!("-----------------");
                                            }
                                            println!("\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#\n#");
                                        }
                                    }
                                }
                            }
                        },
                    }
                }
            },
        }
    }

    
}

fn to_utf8_or_raw(presumably_unicode: &Vec<u8>) -> String {
    match str::from_utf8(presumably_unicode) {
        Err(_e) => format!("Could not deserialize to unicode: {:?}", presumably_unicode),
        Ok(unicode) => unicode.to_string(),
    }
}

fn dmesg_line_newer_than(line: &str, point: &DateTime<Utc>) -> Result<bool, String> {
    let words = line.split_ascii_whitespace();
    for word in words {
        let maybe_time = DateTime::parse_from_rfc3339(word);
        match maybe_time {
            Err(_e) => continue,
            Ok(timestamp) => {
                return Ok(timestamp > DateTime::from(*point))
            },
        }
    }
    Err(format!("Could not parse date from line: {}", line).to_string())
}