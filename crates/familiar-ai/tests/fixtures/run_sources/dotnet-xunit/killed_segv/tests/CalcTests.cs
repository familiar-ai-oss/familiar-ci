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
        System.Runtime.InteropServices.Marshal.WriteInt32(new IntPtr(0x7ff000000000), 1);
    }
}
