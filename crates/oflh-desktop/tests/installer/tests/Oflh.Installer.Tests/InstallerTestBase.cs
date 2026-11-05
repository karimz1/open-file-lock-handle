using FlaUI.Core.AutomationElements;
using NUnit.Framework;
using NUnit.Framework.Interfaces;
using Oflh.Installer.Testing.Core;
using Oflh.Installer.Testing.Infrastructure;

namespace Oflh.Installer.Tests;

/// <summary>Per-test process, UI-automation and report lifetime.</summary>
[Apartment(ApartmentState.STA), NonParallelizable]
public abstract class InstallerTestBase
{
    // Text must sit in a padded content area rather than run to the window edge.
    private const int MinimumTextInsetPixels = 16;

    protected ProcessRunner Processes { get; private set; } = null!;
    protected InstallerWindowDriver Driver { get; private set; } = null!;
    protected static string Reports => TestEnvironment.ReportDirectory(TestContext.CurrentContext.WorkDirectory);

    [SetUp]
    public void StartSession()
    {
        Processes = new ProcessRunner();
        Driver = new InstallerWindowDriver();
    }

    [TearDown]
    public void EndSession()
    {
        if (TestContext.CurrentContext.Result.Outcome.Status == TestStatus.Failed)
            Driver.SaveAllScreenshots(Reports, "failure-" + TestContext.CurrentContext.Test.Name);
        Driver.Dispose();
        Processes.Dispose();
    }

    /// <summary>
    /// Saves the page and checks the branding: native OFLH wordmark, OFLH branding
    /// line instead of NSIS's default, and text inset from the window edge.
    /// </summary>
    protected static void InspectPage(Window window, string screenshotName)
    {
        InstallerWindowDriver.SaveScreenshot(window, Reports, screenshotName);
        var frame = window.BoundingRectangle;
        var brandingLine = InstallerWindowDriver.BrandingLine(window);
        Assert.Multiple(() =>
        {
            Assert.That(InstallerWindowDriver.ShowsBrandingText(window), Is.True, $"{screenshotName}: OFLH branding must be native text.");
            Assert.That(brandingLine, Does.StartWith(InstallerProduct.ProductName), $"{screenshotName}: the OFLH branding line is missing.");
            Assert.That(InstallerWindowDriver.ShowsStockNsisBranding(window), Is.False, $"{screenshotName}: NSIS's default branding is visible.");
            foreach (var (text, bounds) in InstallerWindowDriver.Paragraphs(window))
                Assert.That(frame.Right - bounds.Right, Is.GreaterThanOrEqualTo(MinimumTextInsetPixels),
                    $"{screenshotName}: \"{text}\" runs to the window edge ({bounds}).");
        });
    }
}
