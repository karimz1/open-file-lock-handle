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
            Assert.That(brandingLine, Does.Contain(InstallerProduct.WebsiteLabel).And.Not.Contain(InstallerProduct.StockNsisBranding),
                $"{screenshotName}: the branding line must be OFLH's, not NSIS's default.");
            foreach (var (text, bounds) in InstallerWindowDriver.Paragraphs(window))
                Assert.That(frame.Right - bounds.Right, Is.GreaterThanOrEqualTo(MinimumTextInsetPixels),
                    $"{screenshotName}: \"{text}\" runs to the window edge ({bounds}).");
        });
    }
}
