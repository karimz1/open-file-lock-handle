using NUnit.Framework;
using NUnit.Framework.Interfaces;
using Oflh.Installer.Testing.Core;
using Oflh.Installer.Testing.Infrastructure;

namespace Oflh.Installer.Tests;

/// <summary>Per-test process, UI-automation and report lifetime.</summary>
[Apartment(ApartmentState.STA), NonParallelizable]
public abstract class InstallerTestBase
{
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
}
