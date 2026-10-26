using Xunit;
using System.Diagnostics;
using CalcLib;

namespace Tests;

public class CalcTests
{
    [Fact]
    public void Adds()
    {
        Assert.Equal(4, Calc.Add(2, 2));
    }

    [Fact]
    public void Crashes()
    {
        var psi = new ProcessStartInfo("sh"); psi.ArgumentList.Add("-c"); psi.ArgumentList.Add("kill -KILL " + Environment.ProcessId); Process.Start(psi)!.WaitForExit(); Thread.Sleep(5000);
    }
}
