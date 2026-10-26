using NUnit.Framework;
using CalcLib;

namespace Tests;

[TestFixture]
public class CalcTests
{
    [Test]
    public void Multiplies()
    {
        Assert.That(Calc.Mul(2, 3), Is.EqualTo(6));
    }
}
