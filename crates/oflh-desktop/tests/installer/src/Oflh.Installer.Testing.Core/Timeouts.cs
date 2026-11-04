namespace Oflh.Installer.Testing.Core;

/// <summary>Upper bounds for waits; tests poll and continue as soon as the condition holds.</summary>
public static class Timeouts
{
    public static readonly TimeSpan Window = TimeSpan.FromSeconds(45);
    public static readonly TimeSpan SilentInstall = TimeSpan.FromMinutes(2);
    public static readonly TimeSpan Uninstall = TimeSpan.FromMinutes(1);
    public static readonly TimeSpan WizardCompletion = TimeSpan.FromMinutes(2);
    public static readonly TimeSpan ProcessExit = TimeSpan.FromSeconds(45);
    public static readonly TimeSpan DialogClose = TimeSpan.FromSeconds(10);
    public static readonly TimeSpan PollInterval = TimeSpan.FromMilliseconds(150);
}
