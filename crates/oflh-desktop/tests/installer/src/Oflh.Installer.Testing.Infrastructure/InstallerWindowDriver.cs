using System.Diagnostics;
using System.Drawing;
using System.Runtime.InteropServices;
using FlaUI.Core;
using FlaUI.Core.AutomationElements;
using FlaUI.Core.Capturing;
using FlaUI.Core.Definitions;
using FlaUI.Core.Input;
using FlaUI.Core.Tools;
using FlaUI.Core.WindowsAPI;
using FlaUI.UIA3;
using Oflh.Installer.Testing.Core;

namespace Oflh.Installer.Testing.Infrastructure;

/// <summary>FlaUI wrapper for the NSIS wizard and its owned prompts.</summary>
public sealed class InstallerWindowDriver : IDisposable
{
    // NSIS assigns the Next/Install/Finish button the standard dialog ID 1.
    private const string PrimaryButtonId = "1";

    // Shorter texts are titles, labels and buttons rather than paragraphs.
    private const int MinimumParagraphLength = 40;

    private readonly UIA3Automation automation = new();
    private readonly List<Window> windows = [];

    public Window MainWindowOf(Process process)
    {
        var application = Application.Attach(process);
        var window = WaitFor(() => application.GetAllTopLevelWindows(automation).FirstOrDefault(),
            $"No main window appeared for {process.ProcessName}.", Timeouts.Window);
        windows.Add(window);
        return window;
    }

    public static Button PrimaryButton(Window window) => WaitFor(
        () => window.FindFirstDescendant(condition => condition.ByAutomationId(PrimaryButtonId))?.AsButton(),
        "The installer's primary button was not found.", Timeouts.Window);

    public static Window WaitForModal(Window owner) => WaitFor(
        () => owner.ModalWindows.FirstOrDefault(), "No owned prompt appeared.", Timeouts.Window);

    public static void WaitForModalToClose(Window owner) =>
        Retry.WhileTrue(() => owner.ModalWindows.Length > 0, Timeouts.DialogClose, Timeouts.PollInterval, throwOnTimeout: true);

    public static Button ConfirmButton(Window prompt) => WaitFor(
        () => prompt.FindFirstDescendant(condition => condition.ByControlType(ControlType.Button)
            .And(condition.ByAutomationId("CommandButton_1").Or(condition.ByAutomationId("1")).Or(condition.ByName("OK"))))?.AsButton(),
        "The prompt's confirm button was not found.", Timeouts.Window);

    public static void PressEnter(Window window)
    {
        window.Focus();
        Keyboard.Type(VirtualKeyShort.RETURN);
    }

    public static DialogPlacement PlacementOf(Window prompt, Window owner) =>
        DialogPlacement.Measure(owner.BoundingRectangle, prompt.BoundingRectangle);

    public static bool IsInside(Window prompt, Window owner) =>
        owner.BoundingRectangle.Contains(prompt.BoundingRectangle);

    public static bool ShowsBrandingText(Window window) => window
        .FindAllDescendants(condition => condition.ByName(InstallerProduct.BrandingText))
        .Any(element => !element.IsOffscreen);

    /// <summary>Visible static-text paragraphs; used to check that text sits in a padded container.</summary>
    /// <remarks>
    /// UIA reports MUI's hidden header texts as on-screen, so visibility is
    /// taken from the native window style instead.
    /// </remarks>
    public static IReadOnlyList<Rectangle> ParagraphBounds(Window window) => window
        .FindAllDescendants(condition => condition.ByControlType(ControlType.Text))
        .Where(text => text.Name.Length > MinimumParagraphLength && IsNativelyVisible(text))
        .Select(text => text.BoundingRectangle)
        .ToList();

    private static bool IsNativelyVisible(AutomationElement element) =>
        element.Properties.NativeWindowHandle.TryGetValue(out var handle) && handle != IntPtr.Zero
            ? NativeMethods.IsWindowVisible(handle)
            : !element.IsOffscreen;

    /// <summary>Clicks the primary button until <paramref name="stop"/> returns a value.</summary>
    public static T AdvanceUntil<T>(Window window, Func<T?> stop, Action<int> onPage, string failure, TimeSpan timeout)
        where T : class
    {
        var page = 0;
        return WaitFor(() =>
        {
            if (stop() is { } done) return done;
            onPage(page++);
            var next = PrimaryButton(window);
            if (next.IsEnabled) next.Invoke();
            return null;
        }, failure, timeout);
    }

    public static bool IsFinishButton(Button button) =>
        InstallerProduct.FinishButtonNames.Contains(button.Name.Replace("&", "", StringComparison.Ordinal));

    public static void SaveScreenshot(AutomationElement element, string directory, string name)
    {
        Directory.CreateDirectory(directory);
        Capture.Element(element).ToFile(Path.Combine(directory, name + ".png"));
    }

    /// <summary>Best-effort screenshots of every window touched by the test, for failure diagnosis.</summary>
    public void SaveAllScreenshots(string directory, string prefix)
    {
        foreach (var (window, index) in windows.Select((window, index) => (window, index)))
        {
            try
            {
                SaveScreenshot(window, directory, $"{prefix}-{index}");
            }
            catch (Exception exception) when (exception is InvalidOperationException or COMException or ArgumentException)
            {
                // The window may already be gone; the original failure matters more.
            }
        }
    }

    private static T WaitFor<T>(Func<T?> find, string failure, TimeSpan timeout) where T : class =>
        Retry.WhileNull(find, timeout, Timeouts.PollInterval, throwOnTimeout: true,
            ignoreException: true, timeoutMessage: failure).Result!;

    public void Dispose()
    {
        windows.Clear();
        automation.Dispose();
    }
}
