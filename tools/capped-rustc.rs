//! Standalone Windows build tool; compile with tools/build.ps1, without Cargo.
//! Forward native arguments directly so large dependencies avoid cmd.exe limits.
use std::{env, io, process::{self, Command}};

struct Limits { physical_cores: usize, budget_cores: usize, mask: usize }

#[cfg(windows)]
fn limit_process() -> io::Result<Limits> {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcessorInfo { mask: usize, relationship: u32, data: [u64; 2] }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessAffinityMask(process: *mut c_void, mask: *mut usize, system: *mut usize) -> i32;
        fn SetProcessAffinityMask(process: *mut c_void, mask: usize) -> i32;
        fn SetPriorityClass(process: *mut c_void, priority: u32) -> i32;
        fn GetLogicalProcessorInformation(buffer: *mut ProcessorInfo, length: *mut u32) -> i32;
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
        let mut length = 0_u32;
        GetLogicalProcessorInformation(std::ptr::null_mut(), &mut length);
        let record_size = std::mem::size_of::<ProcessorInfo>();
        if length == 0 || length as usize % record_size != 0 {
            return Err(io::Error::other("unable to read physical CPU topology"));
        }
        let mut topology = vec![ProcessorInfo { mask: 0, relationship: 0, data: [0; 2] }; length as usize / record_size];
        if GetLogicalProcessorInformation(topology.as_mut_ptr(), &mut length) == 0 {
            return Err(io::Error::last_os_error());
        }
        let cores: Vec<_> = topology.iter().filter(|entry| entry.relationship == 0)
            .map(|entry| entry.mask & system).filter(|mask| *mask != 0).collect();
        let budget_cores = cores.len().saturating_sub(2).max(1);
        let mask = cores.iter().take(budget_cores).fold(0_usize, |mask, core| mask | core);
        if mask == 0 || SetProcessAffinityMask(handle, mask) == 0 {
            return Err(io::Error::last_os_error());
        }
        // Normal priority, explicitly requested by the user. All builders use
        // the same physical cores (including their SMT siblings), reserving two.
        if SetPriorityClass(handle, 0x20) == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Limits { physical_cores: cores.len(), budget_cores, mask })
    }
}

#[cfg(not(windows))]
fn limit_process() -> io::Result<Limits> {
    Err(io::Error::other("this build wrapper requires Windows process limits"))
}

fn run() -> io::Result<i32> {
    let limits = limit_process()?;
    let mut args = env::args_os().skip(1);
    let compiler = args.next().ok_or_else(|| io::Error::other("missing compiler executable"))?;
    if compiler == "--print-policy" {
        println!("{{\"physical_cores\":{},\"budget_cores\":{},\"logical_cpus\":{},\"affinity_mask\":{}}}",
            limits.physical_cores, limits.budget_cores, limits.mask.count_ones(), limits.mask);
        return Ok(0);
    }
    let mut command = Command::new(compiler);
    command.args(args);
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x20);
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
