using System.ComponentModel;
using System.Runtime.InteropServices;

namespace Oflh.Installer.Testing.Infrastructure;

/// <summary>
/// Temporarily changes the display scale of every active monitor, the same way the
/// Settings app does, and restores the previous scale on dispose.
/// </summary>
/// <remarks>
/// Uses the DisplayConfig DPI-scale request that Settings itself issues; it applies
/// immediately, so a per-monitor DPI-aware installer started afterwards renders at the
/// new scale without a sign-out.
/// </remarks>
public sealed partial class DisplayScale : IDisposable
{
    // The scale steps Windows offers, in order; the API addresses them by index
    // relative to the monitor's recommended step.
    private static readonly int[] Steps = [100, 125, 150, 175, 200, 225, 250, 300, 350, 400, 450, 500];

    private const int AccessDenied = 5;

    private readonly List<(DisplayConfig.SourceId Source, int RelativeStep)> previous = [];

    private DisplayScale(int percent)
    {
        foreach (var source in DisplayConfig.ActiveSources())
        {
            var current = DisplayConfig.GetScale(source);
            var relative = RelativeStep(percent, current)
                ?? throw new InvalidOperationException($"The display does not offer {percent}% scaling; raise its resolution first.");
            previous.Add((source, current.Current));
            DisplayConfig.SetScale(source, relative);
        }
    }

    /// <summary>Whether every active display offers <paramref name="percent"/> scaling.</summary>
    public static bool IsOffered(int percent) =>
        DisplayConfig.ActiveSources().All(source => RelativeStep(percent, DisplayConfig.GetScale(source)) is not null);

    private static int? RelativeStep(int percent, DisplayConfig.ScaleRange range)
    {
        var target = Array.IndexOf(Steps, percent);
        if (target < 0) throw new ArgumentOutOfRangeException(nameof(percent), percent, "Not a Windows display scale step.");
        var relative = target + range.Minimum; // Minimum is minus the recommended step's index.
        return relative <= range.Maximum ? relative : null;
    }

    /// <summary>
    /// Makes this process per-monitor DPI aware so UI Automation bounds and screenshots
    /// are in physical pixels at every scale. Call once before any automation starts.
    /// </summary>
    public static void UsePhysicalPixels()
    {
        // Access denied means the host already chose an awareness; that is fine.
        if (!NativeMethods.SetProcessDpiAwarenessContext(NativeMethods.PerMonitorAwareV2)
            && Marshal.GetLastPInvokeError() is var error && error != AccessDenied)
            throw new Win32Exception(error);
    }

    /// <summary>Applies <paramref name="percent"/> to all active displays until disposed.</summary>
    public static DisplayScale Apply(int percent) => new(percent);

    public void Dispose()
    {
        foreach (var (source, relativeStep) in previous)
            DisplayConfig.SetScale(source, relativeStep);
        previous.Clear();
    }

    private static partial class DisplayConfig
    {
        private const uint OnlyActivePaths = 0x2;
        private const int GetDpiScale = -3;
        private const int SetDpiScale = -4;

        public readonly record struct SourceId(Luid Adapter, uint Id);

        public readonly record struct ScaleRange(int Minimum, int Current, int Maximum);

        public static List<SourceId> ActiveSources()
        {
            Check(GetDisplayConfigBufferSizes(OnlyActivePaths, out var pathCount, out var modeCount));
            var paths = new PathInfo[pathCount];
            var modes = new ModeInfo[modeCount];
            Check(QueryDisplayConfig(OnlyActivePaths, ref pathCount, paths, ref modeCount, modes, IntPtr.Zero));
            return paths.Take((int)pathCount).Select(path => new SourceId(path.SourceAdapter, path.SourceId)).Distinct().ToList();
        }

        public static ScaleRange GetScale(SourceId source)
        {
            var request = new GetScaleRequest { Header = Header(source, GetDpiScale, Marshal.SizeOf<GetScaleRequest>()) };
            Check(DisplayConfigGetDeviceInfo(ref request));
            return new ScaleRange(request.Minimum, request.Current, request.Maximum);
        }

        public static void SetScale(SourceId source, int relativeStep)
        {
            var request = new SetScaleRequest { Header = Header(source, SetDpiScale, Marshal.SizeOf<SetScaleRequest>()), RelativeStep = relativeStep };
            Check(DisplayConfigSetDeviceInfo(ref request));
        }

        private static DeviceInfoHeader Header(SourceId source, int type, int size) =>
            new() { Type = type, Size = (uint)size, Adapter = source.Adapter, Id = source.Id };

        private static void Check(int error)
        {
            if (error != 0) throw new Win32Exception(error);
        }

        [StructLayout(LayoutKind.Sequential)]
        public readonly record struct Luid(uint Low, int High);

        [StructLayout(LayoutKind.Sequential)]
        private struct DeviceInfoHeader
        {
            public int Type;
            public uint Size;
            public Luid Adapter;
            public uint Id;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct GetScaleRequest
        {
            public DeviceInfoHeader Header;
            public int Minimum;
            public int Current;
            public int Maximum;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct SetScaleRequest
        {
            public DeviceInfoHeader Header;
            public int RelativeStep;
        }

        // DISPLAYCONFIG_PATH_INFO: only the source fields are read; the rest is opaque.
        [StructLayout(LayoutKind.Sequential, Size = 72)]
        private struct PathInfo
        {
            public Luid SourceAdapter;
            public uint SourceId;
        }

        // DISPLAYCONFIG_MODE_INFO is only needed as a correctly sized buffer.
        [StructLayout(LayoutKind.Sequential, Size = 64)]
        private struct ModeInfo
        {
            public uint InfoType;
        }

        [LibraryImport("user32.dll")]
        private static partial int GetDisplayConfigBufferSizes(uint flags, out uint pathCount, out uint modeCount);

        [LibraryImport("user32.dll")]
        private static partial int QueryDisplayConfig(uint flags, ref uint pathCount, [Out] PathInfo[] paths,
            ref uint modeCount, [Out] ModeInfo[] modes, IntPtr currentTopology);

        [LibraryImport("user32.dll")]
        private static partial int DisplayConfigGetDeviceInfo(ref GetScaleRequest request);

        [LibraryImport("user32.dll")]
        private static partial int DisplayConfigSetDeviceInfo(ref SetScaleRequest request);
    }
}
