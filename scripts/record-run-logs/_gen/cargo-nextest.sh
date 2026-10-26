. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/rust-common.sh
RN=cargo-nextest; TC='cargo nextest run'; TV='cargo nextest --version; rustc --version'
gen_all
base setup_error; okt; mk .config/nextest.toml <<'X'
experimental = ["setup-scripts"]

[scripts.setup.db]
command = "sh -c 'echo db unavailable >&2; exit 3'"

[[profile.default.scripts]]
filter = "all()"
setup = "db"
X
rec
base runner_crash_config; okt; mk .config/nextest.toml <<'X'
[profile.default]
retries = "many"
X
rec
