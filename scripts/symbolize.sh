#!/usr/bin/env bash
# Names the code in a Windows crash or error report (run from WSL):
#
#   scripts/symbolize.sh REPORT.log              the build's .pdb from target/dist
#   scripts/symbolize.sh REPORT.log --pdb FILE   any .pdb (a dev build's is beside its exe)
#
# A player's copy of the game has no symbols, so its reports name code as
# meridian.exe+0x1c4e2d. The build's .pdb, which scripts/package-windows.sh keeps
# beside the zip as MeridianConflict-Build<N>-<commit>.pdb, turns those back into
# functions, files and lines. The report's first line names the commit
# ("Meridian Conflict 0.1.0+54da4b549a"), and the .pdb of that commit is found in
# target/dist of the main checkout. Prints the report with each frame named;
# Windows' own dbghelp.dll reads the .pdb.
set -euo pipefail

usage() { sed -n '2,/^set -euo/p' "$0" | grep '^#' | sed 's/^# \{0,1\}//'; }
report=""
pdb=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --pdb) pdb="$2"; shift 2 ;;
        -h|--help) usage; exit 0 ;;
        *) report="$1"; shift ;;
    esac
done
[[ -n $report && -f $report ]] || { usage >&2; exit 2; }

if [[ -z $pdb ]]; then
    commit=$(head -1 "$report" | grep -o '+[0-9a-f]\{7,\}' | head -1 | tr -d '+' || true)
    [[ -n $commit ]] || { echo "no commit in the report's first line; pass --pdb" >&2; exit 1; }
    main=$(dirname "$(cd "$(git rev-parse --git-common-dir)" && pwd)")
    pdb=$(ls "$main"/target/dist/*-"$commit"*.pdb 2>/dev/null | head -1 || true)
    [[ -n $pdb ]] || { echo "no .pdb for $commit in $main/target/dist; pass --pdb" >&2; exit 1; }
fi
[[ -f $pdb ]] || { echo "no such .pdb: $pdb" >&2; exit 1; }

offsets=$(tr -d '\r' <"$report" | grep -o 'meridian\.exe+0x[0-9a-f]\+' | sed 's/meridian\.exe+//' | sort -u)
if [[ -z $offsets ]]; then
    cat "$report"
    exit 0
fi

# dbghelp loads the .pdb at an arbitrary base; each frame's offset is added to it.
names=$(powershell.exe -NoProfile -Command "
\$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class Pdb {
    [DllImport(\"dbghelp.dll\", CharSet = CharSet.Unicode)]
    static extern bool SymInitializeW(IntPtr h, string path, bool invade);
    [DllImport(\"dbghelp.dll\")]
    static extern uint SymSetOptions(uint options);
    [DllImport(\"dbghelp.dll\", CharSet = CharSet.Unicode)]
    static extern ulong SymLoadModuleExW(IntPtr h, IntPtr file, string image, string module,
        ulong at, uint size, IntPtr data, uint flags);
    [DllImport(\"dbghelp.dll\", CharSet = CharSet.Unicode)]
    static extern bool SymFromAddrW(IntPtr h, ulong address, out ulong displacement, IntPtr symbol);
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct Line { public uint Size; public IntPtr Key; public uint Number; public IntPtr File; public ulong Address; }
    [DllImport(\"dbghelp.dll\", CharSet = CharSet.Unicode)]
    static extern bool SymGetLineFromAddrW64(IntPtr h, ulong address, out uint displacement, ref Line line);
    const ulong Base = 0x140000000;
    static readonly IntPtr Session = (IntPtr)0x4D43;
    public static void Load(string pdb) {
        SymSetOptions(0x2 | 0x10);  // undecorated names, line numbers
        if (!SymInitializeW(Session, null, false)) throw new Exception(\"SymInitialize failed\");
        if (SymLoadModuleExW(Session, IntPtr.Zero, pdb, \"meridian\", Base, 0x40000000, IntPtr.Zero, 0) == 0)
            throw new Exception(\"could not load \" + pdb);
    }
    public static string Name(ulong offset) {
        IntPtr symbol = Marshal.AllocHGlobal(88 + 2 * 1024);
        try {
            for (int i = 0; i < 88; i++) Marshal.WriteByte(symbol, i, 0);
            Marshal.WriteInt32(symbol, 0, 88);
            Marshal.WriteInt32(symbol, 80, 1024);
            ulong displacement;
            if (!SymFromAddrW(Session, Base + offset, out displacement, symbol)) return \"?\";
            string name = Marshal.PtrToStringUni(symbol + 84, Marshal.ReadInt32(symbol, 76));
            Line line = new Line();
            line.Size = (uint)Marshal.SizeOf(typeof(Line));
            uint column;
            if (SymGetLineFromAddrW64(Session, Base + offset, out column, ref line))
                name += \"  \" + Marshal.PtrToStringUni(line.File) + \":\" + line.Number;
            return name;
        } finally { Marshal.FreeHGlobal(symbol); }
    }
}
'@
[Pdb]::Load('$(wslpath -w "$pdb")')
foreach (\$o in '$(echo $offsets | tr ' ' ',')'.Split(',')) {
    '{0} {1}' -f \$o, [Pdb]::Name([Convert]::ToUInt64(\$o.Substring(2), 16))
}
" | tr -d '\r')

# The report, each frame followed by its name (paths from the repo root, or the
# standard library's).
NAMES=$names awk '
    BEGIN {
        n = split(ENVIRON["NAMES"], lines, "\n")
        for (i = 1; i <= n; i++) {
            at = index(lines[i], " ")
            if (!at) continue
            text = substr(lines[i], at + 1)
            gsub(/\\/, "/", text)
            sub(/[^ ]*\/crates\//, "crates/", text)
            sub(/[^ ]*\/library\//, "library/", text)
            name[substr(lines[i], 1, at - 1)] = text
        }
    }
    {
        sub(/\r$/, "")
        if (match($0, /meridian\.exe\+0x[0-9a-f]+/)) {
            off = substr($0, RSTART + 13, RLENGTH - 13)
            if (off in name) { print $0 "  " name[off]; next }
        }
        print
    }
' "$report"
