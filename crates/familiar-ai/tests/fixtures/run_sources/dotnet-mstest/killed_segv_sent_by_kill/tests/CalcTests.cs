using Microsoft.VisualStudio.TestTools.UnitTesting;
using System.Diagnostics;
using CalcLib;

namespace Tests;

[TestClass]
public class CalcTests
{
    [TestMethod]
    public void Adds()
    {
        Assert.AreEqual(4, Calc.Add(2, 2));
    }

    [TestMethod]
    public void Crashes()
    {
        var psi = new ProcessStartInfo("sh"); psi.ArgumentList.Add("-c"); psi.ArgumentList.Add("kill -SEGV " + Environment.ProcessId); Process.Start(psi)!.WaitForExit(); Thread.Sleep(5000);
    }
}
