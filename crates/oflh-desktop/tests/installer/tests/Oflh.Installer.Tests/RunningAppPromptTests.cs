using NUnit.Framework;
using Oflh.Installer.Testing.Core;
using Oflh.Installer.Testing.Infrastructure;

namespace Oflh.Installer.Tests;

/// <summary>
/// Runs a tiny NSIS fixture compiled from the production running-app prompt macro,
/// so the prompt's placement and default button are checked in seconds.
/// </summary>
[TestFixture, Category("DialogSmoke")]
public sealed class RunningAppPromptTests : InstallerTestBase
{
    private const string CancelResult = "2";

    [Test]
    public async Task Prompt_stays_over_its_installer_follows_it_and_Enter_cancels()
    {
        DialogPlacement? first = null;
        foreach (var (x, y) in new[] { (70, 60), (320, 200) })
        {
            // Arrange
            var fixture = Processes.Start(TestEnvironment.Required(TestEnvironment.DialogFixtureVariable));
            var installer = Driver.MainWindowOf(fixture);
            installer.Move(x, y);

            // Act
            InstallerWindowDriver.PrimaryButton(installer).Invoke();
            var prompt = InstallerWindowDriver.WaitForModal(installer);
            var placement = InstallerWindowDriver.PlacementOf(prompt, installer);
            InstallerWindowDriver.SaveScreenshot(prompt, Reports, $"running-app-prompt-at-{x}-{y}");
            InstallerWindowDriver.PressEnter(prompt);

            // Assert
            Assert.That(InstallerWindowDriver.IsInside(prompt, installer), Is.True, "The prompt must sit over the installer.");
            if (first is { } previous)
                Assert.That(placement.MatchesRelativeTo(previous), Is.True, "The prompt must follow the installer when it moves.");
            Assert.That(await ProcessRunner.WaitForExitAsync(fixture, Timeouts.ProcessExit), Is.True, "Enter did not dismiss the prompt.");
            Assert.That(await File.ReadAllTextAsync(TestEnvironment.Required(TestEnvironment.DialogResultVariable)), Is.EqualTo(CancelResult),
                "Enter must choose Cancel.");
            first = placement;
        }
    }
}
