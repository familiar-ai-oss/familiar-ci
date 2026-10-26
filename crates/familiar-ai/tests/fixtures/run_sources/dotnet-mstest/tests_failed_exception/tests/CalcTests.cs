using Microsoft.VisualStudio.TestTools.UnitTesting;
using CalcLib;

namespace Tests;

[TestClass]
public class CalcTests
{
    [TestMethod]
    public void Parses()
    {
        Assert.AreEqual(1, Calc.Parse("x1"));
    }
}
