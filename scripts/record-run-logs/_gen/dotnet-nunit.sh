. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/dotnet-common.sh
FW=nunit; RN=dotnet-nunit
gen_core
base setup_error; mk tests/CalcTests.cs <<'X'
using NUnit.Framework;
using CalcLib;

namespace Tests;

[TestFixture]
public class CalcTests
{
    [OneTimeSetUp]
    public void Connect()
    {
        throw new InvalidOperationException("db down");
    }

    [Test]
    public void Adds()
    {
        Assert.That(Calc.Add(2, 2), Is.EqualTo(4));
    }
}
X
rec
# --- cells first merged into dotnet-xunit by reasoning, now recorded with NUnit 4.2.2 + NUnit3TestAdapter 4.6.0 ---
base collection_error; tfile Adds 4 'Calc.Add(2, 2)'; sed -i 's/Assert.That(Calc.Add(2, 2), Is.EqualTo(4));/Assert.That(Calc.Add(2, 2), Is.EqualTo(4))/' tests/CalcTests.cs; rec
base collection_error_import; tfile Adds 4 'Calc.Add(2, 2)'; sed -i '1i using Nosuch.Helpers;' tests/CalcTests.cs; rec
base missing_target_symbol; tfile Multiplies 6 'Calc.Mul(2, 3)'; rec
base usage_error; tfile Adds 4 'Calc.Add(2, 2)'; setcmd "$(D_ 'dotnet test --nosuchflag')"; rec
base usage_error_filter; tfile Adds 4 'Calc.Add(2, 2)'; setcmd "$(D_ 'dotnet test --filter "(Name=Adds"')"; rec
base environment_error_cmd_not_found; tfile Adds 4 'Calc.Add(2, 2)'; setcmd 'dotnet test'; setver 'command -v dotnet || echo "dotnet: not installed on this host (the point of this fixture)"'; rec
base environment_error_missing_dep; tfile Adds 4 'Calc.Add(2, 2)'; sed -i 's#<!--EXTRA-->#<PackageReference Include="Nosuch.Helper.Package.Xyz" Version="9.9.9" />#' Calc.Tests.csproj; rec
base runner_crash; tfile Adds 4 'Calc.Add(2, 2)'; mk Calc.Tests.csproj <<'X'
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net8.0</TargetFramework>
  </PropertyGroup
</Project>
X
rec
base runner_crash_adapter; tfile Adds 4 'Calc.Add(2, 2)'; sed -i '/NUnit3TestAdapter/d' Calc.Tests.csproj; rec
# --- big_run: 600 data-driven cases in 5 classes (tests/Part0Tests.cs .. Part4Tests.cs, 120 each),
#     exactly one failing assertion, in the middle class: Part2Tests, i == 60 ---
bigrun_files() {
  mkdir -p tests
  for f in 0 1 2 3 4; do
    if [ "$f" = 2 ]; then want='i == 60 ? -1 : i + 1'; else want='i + 1'; fi
    {
      U; echo 'using CalcLib;'; echo; echo 'namespace Tests;'; echo; CA; echo "public class Part${f}Tests"; echo '{'
      case $FW in
        xunit) echo '    public static IEnumerable<object[]> Numbers() => Enumerable.Range(0, 120).Select(i => new object[] { i });'; echo
               echo '    [Theory]'; echo '    [MemberData(nameof(Numbers))]'; echo '    public void Adds(int i)';;
        nunit) echo '    [Test]'; echo '    public void Adds([Range(0, 119)] int i)';;
        mstest) echo '    public static IEnumerable<object[]> Numbers() => Enumerable.Range(0, 120).Select(i => new object[] { i });'; echo
               echo '    [TestMethod]'; echo '    [DynamicData(nameof(Numbers), DynamicDataSourceType.Method)]'; echo '    public void Adds(int i)';;
      esac
      echo '    {'; EQ "$want" 'Calc.Add(i, 1)'; echo '    }'; echo '}'
    } > "tests/Part${f}Tests.cs"
  done
}
base big_run; bigrun_files; rec
base big_run_verbose; bigrun_files; setcmd "$(D_ 'dotnet test --logger "console;verbosity=detailed"')"; rec
