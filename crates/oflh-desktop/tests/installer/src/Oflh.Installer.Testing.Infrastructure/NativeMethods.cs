using System.Runtime.InteropServices;

namespace Oflh.Installer.Testing.Infrastructure;

internal static partial class NativeMethods
{
    // IsWindowVisible checks WS_VISIBLE on the window and all of its parents.
    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool IsWindowVisible(IntPtr window);
}
