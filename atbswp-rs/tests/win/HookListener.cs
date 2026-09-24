// Records what Windows actually delivers: low-level keyboard and mouse
// hooks log every event to a file until a stop file appears.
//
//   HookListener.exe LOG STOPFILE
//
// Lines: "key sc=0x1e ext=0 down", "move X Y", "button left down",
//        "wheel -120", "hwheel 120"
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows.Forms;

class HookListener {
    delegate IntPtr HookProc(int code, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")] static extern IntPtr SetWindowsHookEx(int id, HookProc fn, IntPtr mod, uint tid);
    [DllImport("user32.dll")] static extern bool UnhookWindowsHookEx(IntPtr hook);
    [DllImport("user32.dll")] static extern IntPtr CallNextHookEx(IntPtr hook, int code, IntPtr wParam, IntPtr lParam);
    [DllImport("kernel32.dll")] static extern IntPtr GetModuleHandle(string name);

    [StructLayout(LayoutKind.Sequential)] struct KBDLLHOOKSTRUCT { public uint vkCode, scanCode, flags, time; public IntPtr extra; }
    [StructLayout(LayoutKind.Sequential)] struct POINT { public int x, y; }
    [StructLayout(LayoutKind.Sequential)] struct MSLLHOOKSTRUCT { public POINT pt; public uint mouseData, flags, time; public IntPtr extra; }

    static StreamWriter log;
    static HookProc kbProc = KbHook, msProc = MsHook;   // keep delegates alive

    static IntPtr KbHook(int code, IntPtr w, IntPtr l) {
        if (code >= 0) {
            var k = (KBDLLHOOKSTRUCT)Marshal.PtrToStructure(l, typeof(KBDLLHOOKSTRUCT));
            bool ext = (k.flags & 0x01) != 0, up = (k.flags & 0x80) != 0;
            log.WriteLine("key sc=0x{0:x} ext={1} {2}", k.scanCode, ext ? 1 : 0, up ? "up" : "down");
        }
        return CallNextHookEx(IntPtr.Zero, code, w, l);
    }

    static IntPtr MsHook(int code, IntPtr w, IntPtr l) {
        if (code >= 0) {
            var m = (MSLLHOOKSTRUCT)Marshal.PtrToStructure(l, typeof(MSLLHOOKSTRUCT));
            int msg = (int)w;
            short hi = (short)((m.mouseData >> 16) & 0xffff);
            switch (msg) {
                case 0x200: log.WriteLine("move {0} {1}", m.pt.x, m.pt.y); break;
                case 0x201: log.WriteLine("button left down"); break;
                case 0x202: log.WriteLine("button left up"); break;
                case 0x204: log.WriteLine("button right down"); break;
                case 0x205: log.WriteLine("button right up"); break;
                case 0x207: log.WriteLine("button middle down"); break;
                case 0x208: log.WriteLine("button middle up"); break;
                case 0x20B: log.WriteLine("button x{0} down", hi); break;
                case 0x20C: log.WriteLine("button x{0} up", hi); break;
                case 0x20A: log.WriteLine("wheel {0}", hi); break;
                case 0x20E: log.WriteLine("hwheel {0}", hi); break;
                default: log.WriteLine("mouse msg=0x{0:x}", msg); break;
            }
        }
        return CallNextHookEx(IntPtr.Zero, code, w, l);
    }

    static int Main(string[] args) {
        if (args.Length != 2) { Console.Error.WriteLine("usage: HookListener LOG STOPFILE"); return 64; }
        log = new StreamWriter(args[0]) { AutoFlush = true };
        IntPtr mod = GetModuleHandle(null);
        IntPtr kb = SetWindowsHookEx(13 /*WH_KEYBOARD_LL*/, kbProc, mod, 0);
        IntPtr ms = SetWindowsHookEx(14 /*WH_MOUSE_LL*/, msProc, mod, 0);
        if (kb == IntPtr.Zero || ms == IntPtr.Zero) { Console.Error.WriteLine("SetWindowsHookEx failed"); return 1; }
        log.WriteLine("listening");
        var started = DateTime.UtcNow;
        var timer = new Timer { Interval = 100 };
        timer.Tick += (s, e) => {
            if (File.Exists(args[1]) || (DateTime.UtcNow - started).TotalSeconds > 120) Application.Exit();
        };
        timer.Start();
        Application.Run();   // message loop, required for low-level hooks
        UnhookWindowsHookEx(kb);
        UnhookWindowsHookEx(ms);
        log.WriteLine("done");
        log.Close();
        return 0;
    }
}
