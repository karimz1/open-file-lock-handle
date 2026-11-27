using NUnit.Framework;
using Oflh.Installer.Testing.Infrastructure;

namespace Oflh.Installer.Tests;

/// <summary>Process-wide setup that must run before any UI Automation call.</summary>
[SetUpFixture]
public sealed class AssemblySetup
{
    [OneTimeSetUp]
    public void UsePhysicalPixels() => DisplayScale.UsePhysicalPixels();
}
