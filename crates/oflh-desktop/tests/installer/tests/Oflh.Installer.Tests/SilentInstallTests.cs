using NUnit.Framework;
using Oflh.Installer.Testing.Core;
using Oflh.Installer.Testing.Infrastructure;

namespace Oflh.Installer.Tests;

/// <summary>Install and uninstall the real package silently and check what it leaves behind.</summary>
[TestFixture, Category("Installer")]
public sealed class SilentInstallTests : InstallerTestBase
{
    private static string Installer => TestEnvironment.Required(TestEnvironment.InstallerVariable);
    private static string InstallDirectory => Path.Combine(Path.GetTempPath(), "oflh-installer-silent");

    [Test]
    public async Task Silent_install_adds_app_shortcut_uninstall_entry_and_context_menu_and_uninstall_removes_them()
    {
        // Arrange
        var options = new InstallOptions(InstallDirectory, Silent: true);
        var probe = new InstallationProbe(InstallDirectory);

        // Act
        var installExit = await Processes.RunAsync(Installer, options.ToArguments(), Timeouts.SilentInstall);

        // Assert
        Assert.Multiple(() =>
        {
            Assert.That(installExit, Is.Zero);
            Assert.That(probe.ExecutableExists, Is.True, "The application executable was not installed.");
            Assert.That(SystemIntegrationProbe.HasStartMenuShortcut, Is.True, "No Start menu shortcut was created.");
            Assert.That(SystemIntegrationProbe.HasUninstallEntry, Is.True, "No Apps & features entry was registered.");
            Assert.That(SystemIntegrationProbe.ContextMenuClassesRegistered(), Is.EquivalentTo(InstallerProduct.ContextMenuClasses),
                "The context menu must cover files, folders, folder background and drives exactly once each.");
        });

        // Act
        var uninstallExit = await UninstallAsync(probe);

        // Assert
        Assert.Multiple(() =>
        {
            Assert.That(uninstallExit, Is.Zero);
            Assert.That(probe.ExecutableExists, Is.False, "The uninstaller left the executable behind.");
            Assert.That(SystemIntegrationProbe.HasUninstallEntry, Is.False, "The Apps & features entry survived uninstall.");
            Assert.That(SystemIntegrationProbe.ContextMenuClassesRegistered(), Is.Empty, "Context menu entries survived uninstall.");
        });
    }

    [Test]
    public async Task NOCONTEXTMENU_switch_installs_without_context_menu_entries()
    {
        // Arrange
        var options = new InstallOptions(InstallDirectory, Silent: true, ContextMenu: false);
        var probe = new InstallationProbe(InstallDirectory);

        try
        {
            // Act
            var exit = await Processes.RunAsync(Installer, options.ToArguments(), Timeouts.SilentInstall);

            // Assert
            Assert.That(exit, Is.Zero);
            Assert.That(probe.ExecutableExists, Is.True);
            Assert.That(SystemIntegrationProbe.ContextMenuClassesRegistered(), Is.Empty);
        }
        finally
        {
            await UninstallAsync(probe);
        }
    }

    private Task<int> UninstallAsync(InstallationProbe probe) => File.Exists(probe.UninstallerPath)
        ? Processes.RunAsync(probe.UninstallerPath, $"/S _?={InstallDirectory}", Timeouts.Uninstall)
        : Task.FromResult(0);

    [TearDown]
    public void RemoveInstallDirectory()
    {
        if (Directory.Exists(InstallDirectory)) Directory.Delete(InstallDirectory, recursive: true);
    }
}
