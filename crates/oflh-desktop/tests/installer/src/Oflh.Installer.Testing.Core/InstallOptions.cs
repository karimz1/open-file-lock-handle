namespace Oflh.Installer.Testing.Core;

/// <summary>Command-line options passed to the NSIS installer.</summary>
public sealed record InstallOptions(string InstallDirectory, bool Silent, bool ContextMenu = true)
{
    public string ToArguments()
    {
        var arguments = new List<string>();
        if (Silent) arguments.Add("/S");
        if (!ContextMenu) arguments.Add("/NOCONTEXTMENU");
        // NSIS requires /D to be last and unquoted.
        arguments.Add($"/D={InstallDirectory}");
        return string.Join(' ', arguments);
    }
}
