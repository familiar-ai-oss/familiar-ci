. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
IMG=mcr.microsoft.com/dotnet/sdk:8.0
D_() { printf "../../_bin/dock $IMG '%s'" "$1"; }
# FW = xunit | nunit | mstest ; RN = dotnet-$FW
proj() {
  case $FW in
    xunit) pk='    <PackageReference Include="xunit" Version="2.9.2" />
    <PackageReference Include="xunit.runner.visualstudio" Version="2.8.2" />';;
    nunit) pk='    <PackageReference Include="NUnit" Version="4.2.2" />
    <PackageReference Include="NUnit3TestAdapter" Version="4.6.0" />';;
    mstest) pk='    <PackageReference Include="MSTest.TestAdapter" Version="3.6.4" />
    <PackageReference Include="MSTest.TestFramework" Version="3.6.4" />';;
  esac
  cat > Calc.Tests.csproj <<X
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
    <IsPackable>false</IsPackable>
    <IsTestProject>true</IsTestProject>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Microsoft.NET.Test.Sdk" Version="17.11.1" />
$pk
    <!--EXTRA-->
  </ItemGroup>
</Project>
X
}
base() {
  fx "$RN" "$1"
  proj
  mk src/Calc.cs <<'X'
namespace CalcLib;

public static class Calc
{
    public static int Add(int a, int b) => a + b;

    public static int Parse(string s)
    {
        return int.Parse(s);
    }
}
X
  setpre 'rm -rf bin obj TestResults report'
  setver "$(D_ 'dotnet --version')"
  setcmd "$(D_ 'dotnet test')"
}
# test file helpers: usings + class attrs + method attr per framework
U() { case $FW in xunit) echo 'using Xunit;';; nunit) echo 'using NUnit.Framework;';; mstest) echo 'using Microsoft.VisualStudio.TestTools.UnitTesting;';; esac; }
CA() { case $FW in xunit) ;; nunit) echo '[TestFixture]';; mstest) echo '[TestClass]';; esac; }
MA() { case $FW in xunit) echo '    [Fact]';; nunit) echo '    [Test]';; mstest) echo '    [TestMethod]';; esac; }
EQ() { case $FW in xunit) echo "        Assert.Equal($1, $2);";; nunit) echo "        Assert.That($2, Is.EqualTo($1));";; mstest) echo "        Assert.AreEqual($1, $2);";; esac; }
tfile() { # $1 method name, $2 expected, $3 actual  [extra body lines on stdin ignored]
  mkdir -p tests
  { U; echo 'using CalcLib;'; echo; echo 'namespace Tests;'; echo; CA; echo 'public class CalcTests'; echo '{'; MA; echo "    public void $1()"; echo '    {'; EQ "$2" "$3"; echo '    }'; echo '}'; } > tests/CalcTests.cs
}
killfile() { # $1 body
  mkdir -p tests
  { U; echo 'using System.Diagnostics;'; echo 'using CalcLib;'; echo; echo 'namespace Tests;'; echo; CA; echo 'public class CalcTests'; echo '{'; MA; echo '    public void Adds()'; echo '    {'; EQ 4 'Calc.Add(2, 2)'; echo '    }'; echo; MA; echo '    public void Crashes()'; echo '    {'; echo "        $1"; echo '    }'; echo '}'; } > tests/CalcTests.cs
}
gen_core() {
base passed; tfile Adds 4 'Calc.Add(2, 2)'; rec
base tests_failed; tfile Adds 5 'Calc.Add(2, 2)'; rec
base tests_failed_exception; tfile Parses 1 'Calc.Parse("x1")'; rec
base no_tests_collected_file; mkdir -p tests; { U; echo 'using CalcLib;'; echo; echo 'namespace Tests;'; echo; echo 'public class CalcTests'; echo '{'; echo '    public int Helper() => Calc.Add(1, 1);'; echo '}'; } > tests/CalcTests.cs; rec
base no_tests_collected_filter; tfile Adds 4 'Calc.Add(2, 2)'; setcmd "$(D_ 'dotnet test --filter FullyQualifiedName~Nomatch')"; rec
base killed_failfast; killfile 'Environment.FailFast("fatal: giving up");'; rec
base killed_sigkill; killfile 'var psi = new ProcessStartInfo("sh"); psi.ArgumentList.Add("-c"); psi.ArgumentList.Add("kill -KILL " + Environment.ProcessId); Process.Start(psi)!.WaitForExit(); Thread.Sleep(5000);'; rec
base killed_segv; killfile 'System.Runtime.InteropServices.Marshal.WriteInt32(new IntPtr(0x7ff000000000), 1);'; rec
base killed_segv_sent_by_kill; killfile 'var psi = new ProcessStartInfo("sh"); psi.ArgumentList.Add("-c"); psi.ArgumentList.Add("kill -SEGV " + Environment.ProcessId); Process.Start(psi)!.WaitForExit(); Thread.Sleep(5000);'; rec
}
