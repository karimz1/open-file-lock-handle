using NUnit.Framework;
using Oflh.Installer.Testing.Core;
using Oflh.Installer.Testing.Infrastructure;

namespace Oflh.Installer.Tests;

/// <summary>
/// Walks the interactive uninstaller over a running app: the confirm page and the
/// running-app prompt keep OFLH branding, and confirming removes the app.
/// </summary>
[TestFixture, Category("Installer")]
public sealed class UninstallWizardTests : InstallerTestBase
{
    private static string Installer => TestEnvironment.Required(TestEnvironment.InstallerVariable);
    private static string InstallDirectory => Path.Combine(Path.GetTempPath(), "oflh-installer-uninstall");

    [Test]
    public async Task Uninstall_wizard_keeps_branding_and_closes_the_running_app()
    {
        // Arrange
        var probe = new InstallationProbe(InstallDirectory);
        Assert.That(await Processes.RunAsync(Installer, new InstallOptions(InstallDirectory, Silent: true).ToArguments(), Timeouts.SilentInstall), Is.Zero);
        var app = Processes.Start(probe.ExecutablePath);
        _ = Driver.MainWindowOf(app);

        try
        {
            // Act: _?= runs the uninstaller in place, so the test can watch its window.
            var uninstaller = Processes.Start(probe.UninstallerPath, $"_?={InstallDirectory}");
            var window = Driver.MainWindowOf(uninstaller);
            InspectPage(window, "uninstall-confirm");
            InstallerWindowDriver.PrimaryButton(window).Invoke();
            var prompt = InstallerWindowDriver.WaitForModal(window);
            InstallerWindowDriver.SaveScreenshot(prompt, Reports, "uninstall-running-app-prompt");
            InstallerWindowDriver.ConfirmButton(prompt).Invoke();

            // Assert
            Assert.That(await ProcessRunner.WaitForExitAsync(app, Timeouts.ProcessExit), Is.True, "Confirming did not close the running app.");
            // Interactive uninstalls end on the progress page; Close enables when done.
            var close = InstallerWindowDriver.PrimaryButton(window);
            Assert.That(FlaUI.Core.Tools.Retry.WhileFalse(() => close.IsEnabled, Timeouts.Uninstall).Result, Is.True, "The uninstaller never finished removing files.");
            InspectPage(window, "uninstall-finished");
            close.Invoke();
            Assert.That(await ProcessRunner.WaitForExitAsync(uninstaller, Timeouts.Uninstall), Is.True, "The uninstaller did not finish.");
            Assert.Multiple(() =>
            {
                Assert.That(uninstaller.ExitCode, Is.Zero);
                Assert.That(probe.ExecutableExists, Is.False, "The uninstaller left the executable behind.");
                Assert.That(SystemIntegrationProbe.HasUninstallEntry, Is.False, "The Apps & features entry survived uninstall.");
            });
        }
        finally
        {
            Processes.StopStarted();
            ProcessRunner.StopAll(probe.ExecutablePath);
            if (File.Exists(probe.UninstallerPath))
                await Processes.RunAsync(probe.UninstallerPath, $"/S _?={InstallDirectory}", Timeouts.Uninstall);
            InstallationProbe.DeleteDirectory(InstallDirectory);
        }
    }
}
