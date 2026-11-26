using NUnit.Framework;
using Oflh.Installer.Testing.Core;
using Oflh.Installer.Testing.Infrastructure;

namespace Oflh.Installer.Tests;

/// <summary>
/// Installs the latest published release, then this build over it, the way
/// people update from the download page. CI downloads the previous installer
/// fresh on every run; nothing about it is pinned in the repository.
/// </summary>
[TestFixture, Category("Installer")]
public sealed class UpgradeTests : InstallerTestBase
{
    private static string Installer => TestEnvironment.Required(TestEnvironment.InstallerVariable);
    private static string PreviousInstaller => TestEnvironment.Required(TestEnvironment.PreviousInstallerVariable);
    private static string InstallDirectory => Path.Combine(Path.GetTempPath(), "oflh-installer-upgrade");

    [Test]
    public async Task Installing_over_the_latest_release_leaves_one_current_entry_and_uninstall_removes_everything()
    {
        // Arrange
        var options = new InstallOptions(InstallDirectory, Silent: true);
        var probe = new InstallationProbe(InstallDirectory);
        var previousExit = await Processes.RunAsync(PreviousInstaller, options.ToArguments(), Timeouts.SilentInstall);
        Assert.That(previousExit, Is.Zero, "The previous release did not install.");
        var previousVersion = await SilentInstallTests.InstalledVersionAsync(probe);
        TestContext.Out.WriteLine($"Previous release: {previousVersion}");

        // Act
        var upgradeExit = await Processes.RunAsync(Installer, options.ToArguments(), Timeouts.SilentInstall);
        var currentVersion = await SilentInstallTests.InstalledVersionAsync(probe);
        var entry = SystemIntegrationProbe.PerUserUninstallEntry;
        TestContext.Out.WriteLine($"After upgrade: {currentVersion}");

        // Assert
        Assert.That(upgradeExit, Is.Zero);
        Assert.That(entry, Is.Not.Null, "The upgrade removed the Apps & features entry.");
        Assert.Multiple(() =>
        {
            Assert.That(entry!.DisplayVersion, Is.EqualTo(currentVersion),
                "Apps & features still shows the previous version after the upgrade.");
            Assert.That(entry.Publisher, Is.EqualTo(InstallerProduct.Publisher));
            Assert.That(SilentInstallTests.NormalizedDirectory(entry.InstallLocation),
                Is.EqualTo(SilentInstallTests.NormalizedDirectory(InstallDirectory)));
            Assert.That(SystemIntegrationProbe.HasPerMachineUninstallEntry, Is.False);
            Assert.That(SystemIntegrationProbe.StartMenuShortcutCount, Is.EqualTo(1),
                "The upgrade must leave exactly one Start menu shortcut.");
            Assert.That(SystemIntegrationProbe.ContextMenuClassesRegistered(), Is.EquivalentTo(InstallerProduct.ContextMenuClasses),
                "The upgrade must keep the context menu on files, folders, folder background and drives.");
        });

        // Act
        var uninstallExit = await Processes.RunAsync(probe.UninstallerPath, $"/S _?={InstallDirectory}", Timeouts.Uninstall);

        // Assert
        Assert.Multiple(() =>
        {
            Assert.That(uninstallExit, Is.Zero);
            Assert.That(probe.ExecutableExists, Is.False, "The uninstaller left the executable behind.");
            Assert.That(SystemIntegrationProbe.HasUninstallEntry, Is.False, "The Apps & features entry survived uninstall.");
            Assert.That(SystemIntegrationProbe.StartMenuShortcutCount, Is.Zero, "A Start menu shortcut survived uninstall.");
            Assert.That(SystemIntegrationProbe.ContextMenuClassesRegistered(), Is.Empty, "Context menu entries survived uninstall.");
        });
    }

    [TearDown]
    public async Task RemoveInstallation()
    {
        var probe = new InstallationProbe(InstallDirectory);
        if (File.Exists(probe.UninstallerPath))
            await Processes.RunAsync(probe.UninstallerPath, $"/S _?={InstallDirectory}", Timeouts.Uninstall);
        InstallationProbe.DeleteDirectory(InstallDirectory);
    }
}
