using NUnit.Framework;
using CalcLib;

namespace Tests;

[TestFixture]
public class CalcTests
{
    [Test]
    public void Parses()
    {
        Assert.That(Calc.Parse("x1"), Is.EqualTo(1));
    }
}
