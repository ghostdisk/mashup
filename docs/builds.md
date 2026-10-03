# Windows build resource limits and dependency reuse

Every agent builds through the central helper from its own worktree:

```powershell
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target'
& D:\Mashup\tools\build.ps1 build --locked --bin <owned-binary>
```

The user now authorizes twelve Cargo compiler jobs at Normal priority. CPU
affinity uses physical core count minus two, including SMT siblings on the
selected cores. The native helper queries Windows CPU topology rather than
guessing from logical processor count. On the current Ryzen 7 5700G that means
six of eight physical cores / twelve of sixteen logical CPUs. Every compiler,
linker and build script shares the same CPU budget. Rust can create additional
threads, but they execute only on the allowed CPUs. Dev builds only.

`Local\MashupSharedDependencyBuild` serializes Cargo invocations using the new
shared cache; it is separate from the obsolete private-cache queue. Parallel work
occurs within that invocation, avoiding competing cold builds and excessive
duplicate compiler graphs. Application development remains parallel across chats.

## Cache and executable isolation

`CARGO_BUILD_BUILD_DIR` points at ignored `D:\Mashup\user_data\build-cache`.
Cargo keeps compatible dependency variants there: version, features, toolchain,
profile and compiler settings still control freshness and reuse. Dependency
optimization/profile flags are unchanged; no profile-triggered rebuild is needed
just to adjust scheduling.

Final `CARGO_TARGET_DIR` outputs remain private to the worktree. The helper also
sets `RUSTC_WORKSPACE_WRAPPER` to the unique absolute path
`<worktree>\target\mashup-build\workspace-rustc.exe`. Cargo includes that path
in workspace-crate artifact hashes. Application libraries, binaries and
incremental state from divergent checkouts therefore remain separate, while
registry dependencies share their normal cache keys. This addresses the stale
application-library issue previously observed with an unpartitioned shared target.

`seed-build-cache.ps1` copies a completed coordinator dependency snapshot into
the new cache once, under a short bootstrap mutex. It excludes Mashup application entries
and does not modify, hard-link or delete the donor target. Cargo decides which
artifacts remain compatible; changed feature sets may compile a new variant once.
Do not clean the shared cache or alter compiler profiles/flags for a single worker.
Do not manually reuse another worktree's application artifacts.

## Bootstrap and transition

The helper bootstraps `tools/capped-rustc-v2.exe` directly under a conservative CPU
mask at Normal priority while holding the bootstrap gate, then reads its `--print-policy`
output and applies the actual physical-core budget before starting Cargo.
Native argument forwarding avoids cmd.exe's command-length limit. A bootstrap
or topology failure refuses to launch an uncapped compiler.

`& D:\Mashup\tools\build.ps1 prepare` bootstraps the wrapper and seeds the cache
without compiling a game. Its short bootstrap/cache gate is separate from the
Cargo gate, so preparation can finish while an old Cargo build runs. The versioned
wrapper does not replace the old executable used by a running build.

During this coordinated transition, leave active Cargo/rustc builds to finish
unless Coordinator has explicitly coordinated cancellation of an obsolete cold build.
Workers may stop only their own helper invocation that is still waiting for the
mutex, then requeue through the updated helper to adopt dependency reuse and
the new budget. They must not stop another worker's process.
Old already-running invocations retain their initial Cargo job count; future
invocations use the new policy. Games launched afterward are not CPU-capped.

Cargo documents the separate intermediate build directory and workspace-wrapper
hashing in its [configuration reference](https://doc.rust-lang.org/cargo/reference/config.html).
Windows exposes physical-core masks through
[GetLogicalProcessorInformation](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getlogicalprocessorinformation).
