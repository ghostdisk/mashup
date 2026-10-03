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

Renderer-specific dependencies should be opt-in rather than expanding every
game's dependency graph. UI owns separating CEF behind an optional `html-ui`
feature, with action/state types usable without the browser renderer. This
separation is assigned work, not yet a published capability. The first native
demo checkpoint must not wait for CEF integration. An active CEF build can still
populate its legitimate shared feature variant; do not cancel it or alter profiles.

A worker sandbox may permit writes only inside its worktree, while the shared
cache lives under the primary checkout. Use the narrow execution escalation for
the authorized central-helper build when needed; automatic review handles that
filesystem boundary. Do not change cache layout or ask for another routine build
approval. Report an actual automatic-review rejection without bypassing it.

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

The helper prints its own PID, workspace and command at startup. Cancel through
the recorded launch session. Global process listings do not establish ownership;
manual termination requires exact owned binary, parent ancestry from the helper
and start-time verification. A single visible Cargo PID may belong to a different
worker. Do not terminate it merely because your own session is waiting.

Cargo documents the separate intermediate build directory and workspace-wrapper
hashing in its [configuration reference](https://doc.rust-lang.org/cargo/reference/config.html).
Windows exposes physical-core masks through
[GetLogicalProcessorInformation](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getlogicalprocessorinformation).

## Observed verification

The native policy query reported eight physical cores, six selected physical
cores, twelve logical CPUs and affinity mask `4095` (`0xfff`). The coordinator's
first shared-cache `mashup-gtasa-cstrike` dev build completed successfully in
1 minute 32 seconds. Verbose Cargo output marked the full Bevy graph, including
`bevy_pbr`, Fresh and compiled the application library/binary only. The rustc
command used the central versioned wrapper plus the coordinator's distinct
workspace wrapper, and wrote final output to its private target. No game window
was launched and no tests, formatters or linters were run.
