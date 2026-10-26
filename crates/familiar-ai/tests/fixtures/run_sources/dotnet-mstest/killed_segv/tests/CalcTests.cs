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
        System.Runtime.InteropServices.Marshal.WriteInt32(new IntPtr(0x7ff000000000), 1);
    }
}
