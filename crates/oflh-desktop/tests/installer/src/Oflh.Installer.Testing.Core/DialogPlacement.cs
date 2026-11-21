using System.Drawing;

namespace Oflh.Installer.Testing.Core;

/// <summary>Offset of a modal prompt's centre from its owner window's centre.</summary>
public readonly record struct DialogPlacement(float OffsetX, float OffsetY)
{
    private const float TolerancePixels = 2f;

    public static DialogPlacement Measure(Rectangle owner, Rectangle dialog) => new(
        (dialog.Left + dialog.Right - owner.Left - owner.Right) / 2f,
        (dialog.Top + dialog.Bottom - owner.Top - owner.Bottom) / 2f);

    /// <summary>True when both placements keep the same relative offset, i.e. the prompt followed its owner.</summary>
    public bool MatchesRelativeTo(DialogPlacement other) =>
        Math.Abs(OffsetX - other.OffsetX) <= TolerancePixels &&
        Math.Abs(OffsetY - other.OffsetY) <= TolerancePixels;
}
