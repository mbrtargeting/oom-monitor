extern crate chrono;

use sysinfo::{ProcessExt, SystemExt};
use std::{thread, time};
use chrono::{DateTime, Utc};
use std::process::Command;
use std::str;
use std::fmt;

fn main() {
    println!("Hello, world!");

    let tenth_of_a_second = time::Duration::from_millis(100);
    let mut system = sysinfo::System::new();

    loop {
        // First we update all information of our system struct.
        system.refresh_all();

        // Now let's print every process' id and name:
        //for (pid, proc_) in system.get_process_list() {
            //println!("PID:{}\n  Name:{}\n  CMD:{:#?} => status: {:?}", pid,     proc_.name(),proc_.cmd(), proc_.status());
        //}

        // And finally the RAM and SWAP information:
        //println!("Time: {}", Utc::now());
        //println!("  total memory: {} kB", system.get_total_memory());
        //println!("  used memory : {} kB", system.get_used_memory());
        //println!("  total swap  : {} kB", system.get_total_swap());
        //println!("  used swap   : {} kB", system.get_used_swap());

        let last_snapshot_time = Utc::now();

        thread::sleep(tenth_of_a_second);

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
                                if line.contains("killed") {
                                    let is_new = dmesg_line_newer_than(line, &last_snapshot_time);
                                    match is_new {
                                        Err(e) => println!("{}", e),
                                        Ok(false) => continue,
                                        Ok(true) => println!("{}", line)
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