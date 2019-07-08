extern crate chrono;

use sysinfo::{ProcessExt, SystemExt};
use std::{thread, time};
use chrono::{DateTime, Utc};
use std::process::Command;

fn main() {
    println!("Hello, world!");

    let tenth_of_a_second = time::Duration::from_millis(100);
    let mut system = sysinfo::System::new();

    loop {
        // First we update all information of our system struct.
        system.refresh_all();

        // Now let's print every process' id and name:
        for (pid, proc_) in system.get_process_list() {
            println!("PID:{}\n  Name:{}\n  CMD:{:#?} => status: {:?}", pid,     proc_.name(),proc_.cmd(), proc_.status());
        }

        // And finally the RAM and SWAP information:
        println!("Time: {}", Utc::now());
        println!("  total memory: {} kB", system.get_total_memory());
        println!("  used memory : {} kB", system.get_used_memory());
        println!("  total swap  : {} kB", system.get_total_swap());
        println!("  used swap   : {} kB", system.get_used_swap());

        thread::sleep(tenth_of_a_second);

        let maybe_output = Command::new("dmesg").arg("--human").arg("--ctime").arg("--decode").arg("--nopager").output();
        //dmesg --human -T -x
        match maybe_output {
            Err(e) => println!("Could not read from dmesg: {}", e),
            Ok(output) => {
                if !output.status.success() {
                    println!("dmesg failed with error: {:?}", output.stderr)
                } else {
                    println!("Read from dmesg: {:?}", output.stdout)
                }
            },
        }
    }

    
}
