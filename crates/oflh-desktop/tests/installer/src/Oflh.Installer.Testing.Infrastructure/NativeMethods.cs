using System.Runtime.InteropServices;

namespace Oflh.Installer.Testing.Infrastructure;

internal static partial class NativeMethods
{
    // IsWindowVisible checks WS_VISIBLE on the window and all of its parents.
    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool IsWindowVisible(IntPtr window);

    // Makes UI Automation coordinates and screenshots physical pixels at any display scale.
    internal static readonly IntPtr PerMonitorAwareV2 = new(-4);

    [LibraryImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool SetProcessDpiAwarenessContext(IntPtr context);

    // Renders a window into a device context even when other windows cover it.
    internal const uint PrintWindowRenderFullContent = 2;

    [LibraryImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static partial bool PrintWindow(IntPtr window, IntPtr deviceContext, uint flags);
}
