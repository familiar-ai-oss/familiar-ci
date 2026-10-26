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
        Environment.FailFast("fatal: giving up");
    }
}
