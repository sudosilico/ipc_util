use std::env;
use sysinfo::{System, SystemExt};

/// Gets the instance count of the current process name.
pub fn current_process_instance_count() -> usize {
    let current_process_name = env::current_exe()
        .unwrap()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut system = System::new();
    system.refresh_processes();

    system
        .processes_by_exact_name(&current_process_name)
        .count()
}
