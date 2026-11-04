using System.Diagnostics;
using System.Drawing;
using System.Threading;
using FlaUI.Core;
using FlaUI.Core.AutomationElements;
using FlaUI.Core.Capturing;
using FlaUI.Core.Definitions;
using FlaUI.Core.Input;
using FlaUI.Core.Tools;
using FlaUI.Core.WindowsAPI;
using FlaUI.UIA3;
using NUnit.Framework;
using Button = FlaUI.Core.AutomationElements.Button;
using Application = FlaUI.Core.Application;

namespace Oflh.InstallerTests;

[TestFixture, Apartment(ApartmentState.STA), NonParallelizable]
public sealed class InstallerTests
{
    private UIA3Automation automation = null!;
    [SetUp] public void SetUp() => automation = new UIA3Automation();
    private readonly List<Process> processes = [];
    private readonly List<Window> windows = [];
    private string Reports => Environment.GetEnvironmentVariable("OFLH_INSTALLER_REPORT")
        ?? TestContext.CurrentContext.WorkDirectory;

    private static string Required(string name) => Environment.GetEnvironmentVariable(name)
        ?? throw new InvalidOperationException($"Set {name} before running this test category.");

    private Process Start(string path, string arguments = "")
    {
        var process = Process.Start(new ProcessStartInfo(path, arguments) { UseShellExecute = false })
            ?? throw new InvalidOperationException("Could not launch the test application.");
        processes.Add(process);
        return process;
    }

    private static T Await<T>(Func<T?> find, string message, int seconds = 45) where T : class =>
        Retry.WhileNull(find, timeout: TimeSpan.FromSeconds(seconds),
            interval: TimeSpan.FromMilliseconds(150), throwOnTimeout: true,
            ignoreException: true, timeoutMessage: message).Result!;

    private Window Main(Process process, UIA3Automation automation)
    {
        var app = Application.Attach(process);
        var window = Await(() => app.GetAllTopLevelWindows(automation).FirstOrDefault(), "Main window did not appear.");
        windows.Add(window);
        return window;
    }

    private void Screenshot(Window window, string name)
    {
        Directory.CreateDirectory(Reports);
        Capture.Element(window).ToFile(Path.Combine(Reports, name + ".png"));
    }

    private static void Kill(Process process)
    {
        if (!process.HasExited) { process.Kill(entireProcessTree: true); process.WaitForExit(10000); }
    }

    // Windows chooses an optical vertical offset for native task dialogs. Test
    // the user contract: the modal belongs to, fits over, and follows its owner.
    private static PointF Placement(Window owner, Window modal)
    {
        var parent = owner.BoundingRectangle;
        var dialog = modal.BoundingRectangle;
        Assert.That(parent.Contains(dialog), Is.True,
            $"Prompt must stay over its owner. Owner={parent}; prompt={dialog}");
        return new PointF((dialog.Left + dialog.Right - parent.Left - parent.Right) / 2f,
            (dialog.Top + dialog.Bottom - parent.Top - parent.Bottom) / 2f);
    }

    private static void FollowsOwner(PointF first, PointF second)
    {
        Assert.Multiple(() => {
            Assert.That(second.X, Is.EqualTo(first.X).Within(2), "Prompt did not follow horizontal owner movement.");
            Assert.That(second.Y, Is.EqualTo(first.Y).Within(2), "Prompt did not follow vertical owner movement.");
        });
    }

    private static Button Primary(Window window) => Await(
        () => window.FindFirstDescendant(cf => cf.ByAutomationId("1"))?.AsButton(),
        "The installer primary action was not found.");

    [Test, Category("DialogSmoke")]
    public void NativeDialogFollowsOwnerAndEnterCancels()
    {
        PointF? first = null;
        foreach (var position in new[] { new Point(70, 60), new Point(320, 200) })
        {
            var process = Start(Required("OFLH_DIALOG_SMOKE_EXE"));
            var main = Main(process, automation);
            main.Move(position.X, position.Y);
            Primary(main).Click();
            var prompt = Await(() => main.ModalWindows.FirstOrDefault(), "Owned task dialog did not appear.");
            var placement = Placement(main, prompt);
            if (first is { } previous) FollowsOwner(previous, placement);
            first = placement;
            Screenshot(prompt, $"dialog-{position.X}");
            prompt.Focus();
            Keyboard.Press(VirtualKeyShort.RETURN);
            Assert.That(process.WaitForExit(10000), Is.True, "Enter did not dismiss the default Cancel action.");
            Assert.That(process.ExitCode, Is.Zero);
            Assert.That(File.ReadAllText(Required("OFLH_DIALOG_SMOKE_RESULT")), Is.EqualTo("2"));
        }
    }

    private (Process Process, Window Main, Window Prompt, PointF Placement) RunningPrompt(
        string installer, string installDirectory, UIA3Automation automation, Point position, string prefix)
    {
        var page = 0;
        var process = Start(installer, $"/NOCONTEXTMENU /D={installDirectory}");
        var main = Main(process, automation);
        main.Move(position.X, position.Y);
        var prompt = Await(() => {
            if (process.HasExited) throw new InvalidOperationException($"Installer exited early ({process.ExitCode}).");
            var modal = main.ModalWindows.FirstOrDefault();
            if (modal is not null) return modal;
            if (page < 8) Screenshot(main, $"{prefix}-wizard-{page++}");
            var next = Primary(main);
            if (next.IsEnabled) next.Click();
            return null;
        }, "Installer did not show the running-application prompt.");
        var placement = Placement(main, prompt);
        Screenshot(prompt, prefix + "-running-prompt");
        return (process, main, prompt, placement);
    }

    [Test, Category("Installer")]
    public void InstallCancelReinstallAndUninstallTheRealApplication()
    {
        var installer = Required("OFLH_INSTALLER_EXE");
        var installDirectory = Path.Combine(Path.GetTempPath(), "oflh-installer-flaui");
        Directory.CreateDirectory(Reports);
        try
        {
            var initial = Start(installer, $"/S /NOCONTEXTMENU /D={installDirectory}");
            Assert.That(initial.WaitForExit(120000), Is.True, "Silent installation timed out.");
            Assert.That(initial.ExitCode, Is.Zero);
            var executable = Path.Combine(installDirectory, "oflh-desktop.exe");
            Assert.That(File.Exists(executable), Is.True);
            var app = Start(executable);
            _ = Main(app, automation);

            var cancel = RunningPrompt(installer, installDirectory, automation, new Point(70, 60), "cancel");
            cancel.Prompt.Focus();
            Keyboard.Press(VirtualKeyShort.RETURN);
            Retry.WhileTrue(() => cancel.Main.ModalWindows.Any(), timeout: TimeSpan.FromSeconds(10), throwOnTimeout: true);
            Assert.That(app.HasExited, Is.False, "Default Cancel must leave the installed application running.");
            Kill(cancel.Process);

            var accept = RunningPrompt(installer, installDirectory, automation, new Point(320, 200), "confirm");
            FollowsOwner(cancel.Placement, accept.Placement);
            var okay = Await(() => accept.Prompt.FindFirstDescendant(cf => cf.ByControlType(ControlType.Button)
                .And(cf.ByAutomationId("CommandButton_1").Or(cf.ByAutomationId("1")).Or(cf.ByName("OK"))))?.AsButton(),
                "The task dialog confirmation button was not found.");
            okay.Click();
            Assert.That(app.WaitForExit(45000), Is.True, "Confirmed reinstall did not close the running app.");
            Await(() => {
                var button = Primary(accept.Main);
                if (button.Name.Replace("&", "") is "Finish" or "Fertig stellen" or "完成") return button;
                if (button.IsEnabled) button.Click();
                return null;
            }, "Reinstallation did not reach the finish page.", 120);
            Screenshot(accept.Main, "reinstalled");
            Assert.That(File.Exists(executable), Is.True);
            Kill(accept.Process);

            var uninstall = Start(Path.Combine(installDirectory, "uninstall.exe"), $"/S _?={installDirectory}");
            Assert.That(uninstall.WaitForExit(60000), Is.True, "Uninstallation timed out.");
            Assert.That(uninstall.ExitCode, Is.Zero);
            Assert.That(File.Exists(executable), Is.False, "Uninstaller left the application executable behind.");
        }
        finally
        {
            foreach (var process in processes) Kill(process);
            if (Directory.Exists(installDirectory)) Directory.Delete(installDirectory, recursive: true);
        }
    }

    [TearDown]
    public void CaptureFailureAndStopFixtures()
    {
        if (TestContext.CurrentContext.Result.Outcome.Status == NUnit.Framework.Interfaces.TestStatus.Failed)
            foreach (var window in windows)
                try { Screenshot(window, "failure-" + window.Properties.NativeWindowHandle.Value); } catch { /* Keep the original test failure. */ }
        foreach (var process in processes) Kill(process);
        processes.Clear();
        windows.Clear();
        automation.Dispose();
    }
}
