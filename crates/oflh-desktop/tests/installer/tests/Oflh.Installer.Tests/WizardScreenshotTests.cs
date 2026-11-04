using FlaUI.Core.AutomationElements;
using NUnit.Framework;
using Oflh.Installer.Testing.Core;
using Oflh.Installer.Testing.Infrastructure;

namespace Oflh.Installer.Tests;

/// <summary>
/// Captures every wizard page before installation at common display scales, so the
/// branding and text layout can be reviewed for sharpness on HiDPI screens.
/// </summary>
[TestFixture, Category("Installer")]
public sealed class WizardScreenshotTests : InstallerTestBase
{
    private const int MaximumPages = 6;

    [TestCase(100)]
    [TestCase(150)]
    public void Wizard_pages_show_branding_at_display_scale(int percent)
    {
        // Arrange
        Assume.That(DisplayScale.IsOffered(percent), Is.True, $"This display does not offer {percent}% scaling.");
        using var scale = DisplayScale.Apply(percent);
        var installDirectory = Path.Combine(Path.GetTempPath(), "oflh-installer-screenshots");
        var installer = Processes.Start(TestEnvironment.Required(TestEnvironment.InstallerVariable),
            new InstallOptions(installDirectory, Silent: false).ToArguments());
        var window = Driver.MainWindowOf(installer);
        var pages = new List<string>();

        // Act: stop on the page whose primary button would start installing
        InstallerWindowDriver.AdvanceUntil<Button>(window,
            () => InstallerWindowDriver.PrimaryButton(window) is var button && InstallerWindowDriver.IsInstallButton(button) ? button : null,
            page => pages.Add(Capture(window, $"wizard-page-{page}-{percent}")),
            "The wizard did not reach the install step.", Timeouts.Window);
        var lastPage = Capture(window, $"wizard-page-{pages.Count}-{percent}");

        // Assert
        Assert.Multiple(() =>
        {
            Assert.That(pages, Has.Count.InRange(1, MaximumPages), "Unexpected number of wizard pages.");
            Assert.That(InstallerWindowDriver.ShowsBrandingText(window), Is.True, $"{lastPage}: OFLH branding must be native text.");
        });
    }

    private static string Capture(Window window, string name)
    {
        InstallerWindowDriver.SaveScreenshot(window, Reports, name);
        return name;
    }
}
