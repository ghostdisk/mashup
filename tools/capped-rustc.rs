//! Standalone Windows build tool; compile with tools/build.ps1, without Cargo.
//! Forward native arguments directly so large dependencies avoid cmd.exe limits.
use std::{env, io, process::{self, Command}};

#[cfg(windows)]
fn limit_process() -> io::Result<()> {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessAffinityMask(process: *mut c_void, mask: *mut usize, system: *mut usize) -> i32;
        fn SetProcessAffinityMask(process: *mut c_void, mask: usize) -> i32;
        fn SetPriorityClass(process: *mut c_void, priority: u32) -> i32;
    }
    // These calls operate only on our own process. Valid output pointers hold
    // pointer-sized Windows affinity masks; the pseudo handle needs no close.
    unsafe {
        let handle = GetCurrentProcess();
        let mut available = 0_usize;
        let mut system = 0_usize;
        if GetProcessAffinityMask(handle, &mut available, &mut system) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut mask = 0_usize;
        for bit in 0..usize::BITS {
            let cpu = 1_usize << bit;
            if available & cpu != 0 { mask |= cpu; }
            if mask.count_ones() == 6 { break; }
        }
        if mask == 0 || SetProcessAffinityMask(handle, mask) == 0 {
            return Err(io::Error::last_os_error());
        }
        // BELOW_NORMAL_PRIORITY_CLASS. Child affinity is inherited by Windows.
        if SetPriorityClass(handle, 0x4000) == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn limit_process() -> io::Result<()> {
    Err(io::Error::other("this build wrapper requires Windows process limits"))
}

fn run() -> io::Result<i32> {
    limit_process()?;
    let mut args = env::args_os().skip(1);
    let compiler = args.next().ok_or_else(|| io::Error::other("missing compiler executable"))?;
    let mut command = Command::new(compiler);
    command.args(args);
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x4000);
    }
    Ok(command.status()?.code().unwrap_or(1))
}

fn main() {
    match run() {
        Ok(code) => process::exit(code),
        Err(error) => {
            eprintln!("capped-rustc: {error}; refusing to start an uncapped compiler");
            process::exit(1);
        }
    }
}
