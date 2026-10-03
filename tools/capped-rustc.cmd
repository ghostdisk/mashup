@echo off
rem All worktrees share the same six logical CPUs. Linker children inherit affinity.
rem /b creates no extra window; /wait preserves compiler output and exit status.
start "" /b /wait /belownormal /affinity 3f %*
exit /b %errorlevel%
