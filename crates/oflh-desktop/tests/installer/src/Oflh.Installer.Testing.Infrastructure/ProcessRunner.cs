using System.Diagnostics;

namespace Oflh.Installer.Testing.Infrastructure;

/// <summary>Starts processes and guarantees they are stopped when the runner is disposed.</summary>
public sealed class ProcessRunner : IDisposable
{
    private readonly List<Process> started = [];

    public Process Start(string path, string arguments = "")
    {
        var process = Process.Start(new ProcessStartInfo(path, arguments) { UseShellExecute = false })
            ?? throw new InvalidOperationException($"Could not start {path}.");
        started.Add(process);
        return process;
    }

    /// <summary>Runs a process to completion and returns its exit code.</summary>
    public async Task<int> RunAsync(string path, string arguments, TimeSpan timeout)
    {
        var process = Start(path, arguments);
        using var cancellation = new CancellationTokenSource(timeout);
        await process.WaitForExitAsync(cancellation.Token).ConfigureAwait(false);
        return process.ExitCode;
    }

    public static async Task<bool> WaitForExitAsync(Process process, TimeSpan timeout)
    {
        using var cancellation = new CancellationTokenSource(timeout);
        try
        {
            await process.WaitForExitAsync(cancellation.Token).ConfigureAwait(false);
            return true;
        }
        catch (OperationCanceledException)
        {
            return false;
        }
    }

    public static void Stop(Process process)
    {
        if (process.HasExited) return;
        process.Kill(entireProcessTree: true);
        process.WaitForExit();
    }

    /// <summary>
    /// Stops every running copy of <paramref name="executablePath"/>, including ones
    /// started by the installer itself, such as the Finish page's "Start" option.
    /// </summary>
    public static void StopAll(string executablePath)
    {
        foreach (var process in Process.GetProcessesByName(Path.GetFileNameWithoutExtension(executablePath)))
        {
            using (process)
            {
                if (IsRunningFrom(process, executablePath)) Stop(process);
            }
        }
    }

    private static bool IsRunningFrom(Process process, string executablePath)
    {
        try
        {
            return string.Equals(process.MainModule?.FileName, Path.GetFullPath(executablePath), StringComparison.OrdinalIgnoreCase);
        }
        catch (System.ComponentModel.Win32Exception)
        {
            return false; // Exited meanwhile or belongs to another session.
        }
    }

    /// <summary>Stops every process this runner started that is still running.</summary>
    public void StopStarted()
    {
        foreach (var process in started) Stop(process);
    }

    public void Dispose()
    {
        foreach (var process in started)
        {
            Stop(process);
            process.Dispose();
        }
        started.Clear();
    }
}
