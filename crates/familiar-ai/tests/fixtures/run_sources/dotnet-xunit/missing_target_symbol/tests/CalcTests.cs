using Xunit;
using CalcLib;

namespace Tests;

public class CalcTests
{
    [Fact]
    public void Multiplies()
    {
        Assert.Equal(6, Calc.Mul(2, 3));
    }
}
