namespace Oflh.Installer.Testing.Core;

/// <summary>Inputs supplied by CI through environment variables.</summary>
public static class TestEnvironment
{
    public const string InstallerVariable = "OFLH_INSTALLER_EXE";
    public const string DialogFixtureVariable = "OFLH_DIALOG_SMOKE_EXE";
    public const string DialogResultVariable = "OFLH_DIALOG_SMOKE_RESULT";
    public const string ReportVariable = "OFLH_INSTALLER_REPORT";

    public static string Required(string variable) =>
        Environment.GetEnvironmentVariable(variable)
        ?? throw new InvalidOperationException($"Set {variable} before running this test category.");

    public static string ReportDirectory(string fallback) =>
        Environment.GetEnvironmentVariable(ReportVariable) ?? fallback;
}
