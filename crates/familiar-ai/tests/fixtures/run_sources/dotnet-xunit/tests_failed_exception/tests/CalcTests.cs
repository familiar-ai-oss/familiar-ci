using Xunit;
using CalcLib;

namespace Tests;

public class CalcTests
{
    [Fact]
    public void Parses()
    {
        Assert.Equal(1, Calc.Parse("x1"));
    }
}
