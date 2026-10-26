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
        System.Runtime.InteropServices.Marshal.WriteInt32(new IntPtr(0x7ff000000000), 1);
    }
}
