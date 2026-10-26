using LabLib;
using Xunit;

namespace Tests;

public class GreetingTests
{
    [Fact]
    public void GreetSaysHello()
    {
        Assert.Equal("hello", Greeter.Greet());
    }
}
