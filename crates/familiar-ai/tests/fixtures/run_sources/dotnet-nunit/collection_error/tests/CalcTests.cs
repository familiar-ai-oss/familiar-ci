using NUnit.Framework;
using CalcLib;

namespace Tests;

[TestFixture]
public class CalcTests
{
    [Test]
    public void Adds()
    {
        Assert.That(Calc.Add(2, 2), Is.EqualTo(4))
    }
}
