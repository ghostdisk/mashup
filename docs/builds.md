# Windows build resource limits

After the first helper build, normal Cargo commands in this checkout load
`.cargo/config.toml`: two parallel
compiler jobs, and a Windows rustc wrapper that restricts compilation to eight
logical CPUs (`0xff`) at BelowNormal priority. Compiler child processes, including
the linker, inherit the CPU affinity. All worktrees use the same CPU mask, so
overlapping capped compilers cannot consume the remaining CPUs. On the current
16-logical-CPU machine this reserves half the logical CPUs outside build affinity.

Agents also use the shared build gate to limit memory pressure:

```powershell
& D:\Mashup\tools\build.ps1 build --locked --bin mashup-gtasa-cstrike
```

Run this from the owning checkout/worktree. It waits on a named Windows mutex,
so only one agent build runs at once, sets Cargo jobs to two and applies the
same CPU/priority limits before starting Cargo. The wrapper path is explicit,
so even a worktree that has not fetched the configuration stays capped.
The helper first compiles `tools/capped-rustc.rs` directly under the same limits
when its ignored `.exe` is missing or outdated. The native wrapper forwards
arguments directly to rustc, preserving quoted arguments and avoiding the batch
command-length limit encountered when compiling the large `windows` crate.
Use a worktree-local `CARGO_TARGET_DIR` to avoid sharing mutable library caches.
After building, launch the executable normally; the game is not CPU-capped.

CPU affinity limits simultaneous execution to eight shared logical CPUs. Rust may still
create additional idle/background threads; thread counts do not override the
affinity restriction. The Cargo configuration selects a native Windows wrapper;
non-Windows builds must replace that wrapper with their platform's equivalent.
No compiler flags or code-generation settings change, avoiding a full dependency
rebuild solely to install these process limits.

The user increased this policy from six CPUs/one job on 2026-10-03 to improve
queue throughput while keeping the machine usable. Existing running or queued
helper invocations may retain the earlier limits; leave them to finish. New
invocations use the increased budget. The wrapper is rebuilt only after acquiring
the shared gate, so its executable is not replaced during an active gated build.

Cargo's job and wrapper settings are documented in the
[Cargo configuration reference](https://doc.rust-lang.org/cargo/reference/config.html).
Windows documents inherited processor affinity in
[SetProcessAffinityMask](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-setprocessaffinitymask).
