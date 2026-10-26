using Microsoft.VisualStudio.TestTools.UnitTesting;
using CalcLib;

namespace Tests;

[TestClass]
public class CalcTests
{
    [TestMethod]
    public void Multiplies()
    {
        Assert.AreEqual(6, Calc.Mul(2, 3));
    }
}
