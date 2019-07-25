extern crate chrono;

use sysinfo::{ProcessExt, SystemExt, Pid, Process};
use std::{thread, time};
use chrono::{DateTime, Utc};
use std::process::Command;
use std::str;
use std::fmt;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone)]
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

        snapshots.truncate(20);

        let current_system_state = SystemState {
            timestamp: Utc::now(),
            total_memory: system.get_total_memory(),
            used_memory: system.get_used_memory(),
            total_swap: system.get_total_swap(),
            used_swap: system.get_used_swap(),
            processes: system.get_process_list().to_owned(),
        };
        snapshots.push_front(current_system_state);

        let last_snapshot_time = Utc::now();

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
                                    let is_new = dmesg_line_newer_than(line, &snapshots.back().unwrap().timestamp);
                                    match is_new {
                                        Err(e) => println!("{}", e),
                                        Ok(false) => continue,
                                        Ok(true) => {
                                            let snapshots_to_print = snapshots.clone();
                                            for snapshot in snapshots_to_print {
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