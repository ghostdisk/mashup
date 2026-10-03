param(
    [ValidateRange(1,10000)][int]$Samples = 200,
    [ValidateRange(1,1000)][int]$IntervalMs = 10,
    [string]$Output = 'user_data/reference/movement-state.json'
)
$ErrorActionPreference = 'Stop'
# Read-only observation of the offline process recorded by reference-game.ps1.
# Layout and pointer RVA were independently located in this exact local binary.
$session = Get-Content user_data/reference/session.json -Raw | ConvertFrom-Json
if (!$session.lan) { throw 'The recorded reference session must be offline/LAN.' }
$process = Get-Process -Id $session.pid
if ($process.ProcessName -ne 'hl' -or [Math]::Abs(($process.StartTime.ToUniversalTime()-[DateTime]::Parse($session.started).ToUniversalTime()).TotalSeconds) -gt 3) {
    throw 'Recorded PID no longer belongs to the reference session.'
}
$module = $process.Modules | Where-Object { $_.ModuleName -eq 'mp.dll' } | Select-Object -First 1
if (!$module) { throw 'The reference process has no loaded Counter-Strike server module.' }
$expectedModule = [IO.Path]::GetFullPath((Join-Path $session.installation 'cstrike/dlls/mp.dll'))
if (![String]::Equals($module.FileName,$expectedModule,[StringComparison]::OrdinalIgnoreCase)) {
    throw 'The loaded server module does not belong to the recorded installation.'
}
$hash = (Get-FileHash -LiteralPath $module.FileName -Algorithm SHA256).Hash
if ($hash -ne '6467586318F408B04B2EB0700CB8A8577F146F6124FD2C26A404AD0E4528C723') {
    throw 'Unrecognized server binary. Locate its movement layout in Ghidra before recording; this tool will not guess addresses.'
}
Add-Type @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;
public static class ReferenceMovementReader {
    [DllImport("kernel32.dll",SetLastError=true)] private static extern IntPtr OpenProcess(uint access,bool inherit,int pid);
    [DllImport("kernel32.dll",SetLastError=true)] private static extern bool ReadProcessMemory(IntPtr process,IntPtr address,byte[] buffer,UIntPtr size,out UIntPtr read);
    [DllImport("kernel32.dll")] private static extern bool CloseHandle(IntPtr handle);
    private static byte[] Read(IntPtr process,long address,int size) {
        var data = new byte[size]; UIntPtr count;
        if (!ReadProcessMemory(process,new IntPtr(address),data,(UIntPtr)size,out count) || count.ToUInt64()!=(ulong)size)
            throw new InvalidOperationException("Reference memory read failed: "+Marshal.GetLastWin32Error());
        return data;
    }
    private static float Float(byte[] data,int offset) {
        float value=BitConverter.ToSingle(data,offset);
        if (Single.IsNaN(value) || Single.IsInfinity(value)) throw new InvalidOperationException("Non-finite reference movement value");
        return value;
    }
    private static float[] Vector(byte[] data,int offset) {
        return new float[]{Float(data,offset),Float(data,offset+4),Float(data,offset+8)};
    }
    public static object[] Capture(int pid,long moduleBase,int samples,int interval) {
        // PROCESS_QUERY_INFORMATION | PROCESS_VM_READ. No writes, hooks or code injection.
        IntPtr process=OpenProcess(0x410,false,pid);
        if (process==IntPtr.Zero) throw new InvalidOperationException("Cannot open reference process for reading");
        try {
            var frames=new List<object>(); var clock=Stopwatch.StartNew();
            for (int i=0;i<samples;i++) {
                long state=BitConverter.ToUInt32(Read(process,moduleBase+0x132704,4),0);
                if (state==0) throw new InvalidOperationException("Movement callback has not run; enter the offline match first");
                byte[] command=Read(process,state+0x45458,52);
                byte[] core=Read(process,state,0x214);
                byte[] after=Read(process,state+0x45458,52);
                bool changed=false;
                for (int j=0;j<command.Length;j++) changed |= command[j]!=after[j];
                if (BitConverter.ToInt32(core,4)!=1) throw new InvalidOperationException("Expected server movement state");
                frames.Add(new Dictionary<string,object> {
                    {"sample_time_seconds",clock.Elapsed.TotalSeconds},
                    {"movement_time_raw",Float(core,0x0c)},
                    {"frame_seconds",Float(core,0x10)},
                    {"position_source_units",Vector(core,0x38)},
                    {"velocity_source_units",Vector(core,0x5c)},
                    {"view_offset_source_units",Vector(core,0x80)},
                    {"hull",BitConverter.ToInt32(core,0xbc)},
                    {"flags",BitConverter.ToInt32(core,0xb8)},
                    {"ground_entity",BitConverter.ToInt32(core,0xe0)},
                    {"dead",BitConverter.ToInt32(core,0xd0)},
                    {"movement_type",BitConverter.ToInt32(core,0xdc)},
                    {"duck_timer_ms",Float(core,0x8c)},
                    {"duck_transition",BitConverter.ToInt32(core,0x90)},
                    {"stamina_ms",Float(core,0x210)},
                    {"max_speed_source_units",Float(core,0x1f4)},
                    {"command_msec",(int)command[2]},
                    {"command_angles_degrees",Vector(command,4)},
                    {"command_movement_source_units",Vector(command,16)},
                    {"command_buttons",(int)BitConverter.ToUInt16(command,30)},
                    {"command_changed_during_read",changed}
                });
                if (i+1<samples) Thread.Sleep(interval);
            }
            return frames.ToArray();
        } finally { CloseHandle(process); }
    }
}
'@
$frames = [ReferenceMovementReader]::Capture($process.Id,$module.BaseAddress.ToInt64(),$Samples,$IntervalMs)
$capture = [ordered]@{
    version=1
    coordinate_system='goldsrc_xyz_source_units'
    observation='Live polling; samples can repeat, skip commands, or observe a command in progress. These are not atomic command-boundary snapshots.'
    pid=$process.Id
    started=$session.started
    server_sha256=$hash
    movement_pointer_rva='0x132704'
    requested_interval_ms=$IntervalMs
    frames=@($frames)
}
$absolute = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Force ([IO.Path]::GetDirectoryName($absolute)) | Out-Null
$capture | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $absolute -Encoding utf8
Write-Output $absolute
