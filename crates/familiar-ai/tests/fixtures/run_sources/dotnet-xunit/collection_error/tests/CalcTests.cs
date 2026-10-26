using Xunit;
using CalcLib;

namespace Tests;

public class CalcTests
{
    [Fact]
    public void Adds()
    {
        Assert.Equal(4, Calc.Add(2, 2))
    }
}
