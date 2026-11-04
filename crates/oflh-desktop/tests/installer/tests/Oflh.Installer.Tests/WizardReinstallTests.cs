using System.Drawing;
using FlaUI.Core.AutomationElements;
using NUnit.Framework;
using Oflh.Installer.Testing.Core;
using Oflh.Installer.Testing.Infrastructure;

namespace Oflh.Installer.Tests;

/// <summary>
/// Walks the interactive wizard over a running app: capture every page, cancel the
/// running-app prompt, then confirm and reinstall to the Finish page.
/// </summary>
[TestFixture, Category("Installer")]
public sealed class WizardReinstallTests : InstallerTestBase
{
    // Text must sit in a padded content area rather than run to the window edge.
    private const int MinimumTextInsetPixels = 16;
    private const int MaximumPagesBeforePrompt = 8;

    private static string Installer => TestEnvironment.Required(TestEnvironment.InstallerVariable);
    private static string InstallDirectory => Path.Combine(Path.GetTempPath(), "oflh-installer-wizard");

    [Test]
    public async Task Running_app_prompt_defaults_to_cancel_and_confirming_reinstalls_to_finish()
    {
        // Arrange
        var probe = new InstallationProbe(InstallDirectory);
        var wizardOptions = new InstallOptions(InstallDirectory, Silent: false);
        Assert.That(await Processes.RunAsync(Installer, new InstallOptions(InstallDirectory, Silent: true).ToArguments(), Timeouts.SilentInstall), Is.Zero);
        var app = Processes.Start(probe.ExecutablePath);
        _ = Driver.MainWindowOf(app);

        try
        {
            // Act: cancel at the running-app prompt
            var (cancelWizard, cancelPrompt, cancelPlacement) = OpenWizardToRunningAppPrompt(wizardOptions, new Point(70, 60), "cancel");
            InstallerWindowDriver.PressEnter(cancelPrompt);
            InstallerWindowDriver.WaitForModalToClose(cancelWizard.Window);

            // Assert
            Assert.That(app.HasExited, Is.False, "Cancel must leave the running application alone.");
            ProcessRunner.Stop(cancelWizard.Process);

            // Act: confirm at the running-app prompt
            var (confirmWizard, confirmPrompt, confirmPlacement) = OpenWizardToRunningAppPrompt(wizardOptions, new Point(320, 200), "confirm");
            InstallerWindowDriver.ConfirmButton(confirmPrompt).Invoke();
            var finish = InstallerWindowDriver.AdvanceUntil(confirmWizard.Window,
                () => InstallerWindowDriver.PrimaryButton(confirmWizard.Window) is var button && InstallerWindowDriver.IsFinishButton(button) ? button : null,
                _ => { }, "The reinstall did not reach the Finish page.", Timeouts.WizardCompletion);
            InspectPage(confirmWizard.Window, "wizard-finish");

            // Assert
            Assert.That(confirmPlacement.MatchesRelativeTo(cancelPlacement), Is.True, "The prompt must follow the installer window.");
            Assert.That(await ProcessRunner.WaitForExitAsync(app, Timeouts.ProcessExit), Is.True, "Confirming did not close the running app.");
            Assert.That(finish.IsEnabled, Is.True);
            Assert.That(probe.ExecutableExists, Is.True);
            finish.Invoke();
        }
        finally
        {
            if (File.Exists(probe.UninstallerPath))
                await Processes.RunAsync(probe.UninstallerPath, $"/S _?={InstallDirectory}", Timeouts.Uninstall);
        }
    }

    private ((System.Diagnostics.Process Process, Window Window) Wizard, Window Prompt, DialogPlacement Placement)
        OpenWizardToRunningAppPrompt(InstallOptions options, Point position, string screenshotPrefix)
    {
        var process = Processes.Start(Installer, options.ToArguments());
        var window = Driver.MainWindowOf(process);
        window.Move(position.X, position.Y);
        var prompt = InstallerWindowDriver.AdvanceUntil(window,
            () => window.ModalWindows.FirstOrDefault(),
            page => { if (page < MaximumPagesBeforePrompt) InspectPage(window, $"{screenshotPrefix}-wizard-page-{page}"); },
            "The installer did not show the running-app prompt.", Timeouts.Window);
        Assert.That(InstallerWindowDriver.IsInside(prompt, window), Is.True, "The prompt must sit over the installer.");
        InstallerWindowDriver.SaveScreenshot(prompt, Reports, $"{screenshotPrefix}-running-app-prompt");
        return ((process, window), prompt, InstallerWindowDriver.PlacementOf(prompt, window));
    }

    private static void InspectPage(Window window, string screenshotName)
    {
        InstallerWindowDriver.SaveScreenshot(window, Reports, screenshotName);
        var frame = window.BoundingRectangle;
        Assert.Multiple(() =>
        {
            Assert.That(InstallerWindowDriver.ShowsBrandingText(window), Is.True, $"{screenshotName}: OFLH branding must be native text.");
            foreach (var paragraph in InstallerWindowDriver.ParagraphBounds(window))
                Assert.That(frame.Right - paragraph.Right, Is.GreaterThanOrEqualTo(MinimumTextInsetPixels),
                    $"{screenshotName}: text runs to the window edge ({paragraph}).");
        });
    }
}
