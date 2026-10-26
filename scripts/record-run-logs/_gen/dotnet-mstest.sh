. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/dotnet-common.sh
FW=mstest; RN=dotnet-mstest
gen_core
base setup_error; mk tests/CalcTests.cs <<'X'
using Microsoft.VisualStudio.TestTools.UnitTesting;
using CalcLib;

namespace Tests;

[TestClass]
public class CalcTests
{
    [ClassInitialize]
    public static void Connect(TestContext context)
    {
        throw new InvalidOperationException("db down");
    }

    [TestMethod]
    public void Adds()
    {
        Assert.AreEqual(4, Calc.Add(2, 2));
    }
}
X
rec
base collection_error; tfile Adds 4 'Calc.Add(2, 2)'; sed -i 's/Assert.AreEqual(4, Calc.Add(2, 2));/Assert.AreEqual(4, Calc.Add(2, 2))/' tests/CalcTests.cs; rec
base missing_target_symbol; tfile Multiplies 6 'Calc.Mul(2, 3)'; rec
base usage_error; tfile Adds 4 'Calc.Add(2, 2)'; setcmd "$(D_ 'dotnet test --nosuchflag')"; rec
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
