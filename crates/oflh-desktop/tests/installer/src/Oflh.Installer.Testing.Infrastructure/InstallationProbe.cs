using Microsoft.Win32;
using Oflh.Installer.Testing.Core;

namespace Oflh.Installer.Testing.Infrastructure;

/// <summary>Reads what an installation left in its install directory.</summary>
public sealed class InstallationProbe(string installDirectory)
{
    public string ExecutablePath => Path.Combine(installDirectory, InstallerProduct.ExecutableName);
    public string UninstallerPath => Path.Combine(installDirectory, InstallerProduct.UninstallerName);

    public bool ExecutableExists => File.Exists(ExecutablePath);
}

/// <summary>Reads the shell and registry integration the installer registers for the current user or machine.</summary>
public static class SystemIntegrationProbe
{
    /// <summary>The per-user or per-machine "Apps &amp; features" entry, if any.</summary>
    public static bool HasUninstallEntry => OpenFromEitherHive(InstallerProduct.UninstallRegistryKey) is { } key && Dispose(key);

    public static bool HasStartMenuShortcut => StartMenuRoots()
        .Where(Directory.Exists)
        .Any(root => Directory.EnumerateFiles(root, InstallerProduct.ShortcutName, SearchOption.AllDirectories).Any());

    /// <summary>Shell classes that currently carry the OFLH context-menu verb.</summary>
    public static IReadOnlyList<string> ContextMenuClassesRegistered() => InstallerProduct.ContextMenuClasses
        .Where(shellClass => OpenFromEitherHive($@"Software\Classes\{shellClass}\shell\{InstallerProduct.ContextMenuVerb}") is { } key && Dispose(key))
        .ToList();

    private static RegistryKey? OpenFromEitherHive(string path) =>
        Registry.CurrentUser.OpenSubKey(path) ?? Registry.LocalMachine.OpenSubKey(path);

    private static bool Dispose(RegistryKey key)
    {
        key.Dispose();
        return true;
    }

    private static IEnumerable<string> StartMenuRoots() =>
    [
        Environment.GetFolderPath(Environment.SpecialFolder.Programs),
        Environment.GetFolderPath(Environment.SpecialFolder.CommonPrograms),
    ];
}
