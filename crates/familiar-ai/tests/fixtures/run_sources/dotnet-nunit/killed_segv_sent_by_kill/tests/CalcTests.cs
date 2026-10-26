using NUnit.Framework;
using System.Diagnostics;
using CalcLib;

namespace Tests;

[TestFixture]
public class CalcTests
{
    [Test]
    public void Adds()
    {
        Assert.That(Calc.Add(2, 2), Is.EqualTo(4));
    }

    [Test]
    public void Crashes()
    {
        var psi = new ProcessStartInfo("sh"); psi.ArgumentList.Add("-c"); psi.ArgumentList.Add("kill -SEGV " + Environment.ProcessId); Process.Start(psi)!.WaitForExit(); Thread.Sleep(5000);
    }
}
