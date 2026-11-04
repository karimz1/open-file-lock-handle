namespace Oflh.Installer.Testing.Core;

/// <summary>Facts about the shipped OFLH installer that the tests verify.</summary>
public static class InstallerProduct
{
    public const string ProductName = "OFLH Desktop";
    public const string ExecutableName = "oflh-desktop.exe";
    public const string UninstallerName = "uninstall.exe";
    public const string ShortcutName = ProductName + ".lnk";
    public const string BrandingText = "OFLH";
    public const string UninstallRegistryKey = @"Software\Microsoft\Windows\CurrentVersion\Uninstall\" + ProductName;
    public const string ContextMenuVerb = "oflh-desktop.inspect";

    /// <summary>Shell classes that receive the classic Explorer context-menu entry.</summary>
    public static IReadOnlyList<string> ContextMenuClasses { get; } =
        ["*", "Directory", "Directory\\Background", "Drive"];

    /// <summary>Accessible names of the wizard's last-page button in the shipped languages.</summary>
    public static IReadOnlySet<string> FinishButtonNames { get; } =
        new HashSet<string>(StringComparer.Ordinal) { "Finish", "Fertig stellen", "完成" };

}
