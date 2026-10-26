#!/usr/bin/env python3
"""capture.py FIXTURE_DIR

Runs `sh -c "$(cat cmd)"` with cwd=FIXTURE_DIR and stdin=/dev/null, the way the
product runs a test command, and writes next to it:
  stdout, stderr  - the two streams, captured separately (bytes as emitted)
  exit            - sh's status as the parent sees it: N for a normal exit,
                    -N when the sh process itself was terminated by signal N
                    (bash execs a lone/last command, so a runner that dies by a
                    signal kills "sh" directly: Rust ExitStatus::code()==None,
                    signal()==Some(N)); "timeout" if the time-box hit.
  status          - the same in words
  version         - output of `sh -c "$(cat version.cmd)"` (stdout+stderr)
  elapsed         - wall seconds
"""
import os, signal, subprocess, sys, time

d = os.path.abspath(sys.argv[1])
cmd = open(os.path.join(d, 'cmd')).read().rstrip('\n')
limit = float(os.environ.get('CAPTURE_TIMEOUT', '1500'))
t0 = time.time()
with open(os.path.join(d, 'stdout'), 'wb') as o, open(os.path.join(d, 'stderr'), 'wb') as e:
    p = subprocess.Popen(['sh', '-c', cmd], cwd=d, stdin=subprocess.DEVNULL,
                         stdout=o, stderr=e, start_new_session=True)
    try:
        rc = p.wait(timeout=limit)
    except subprocess.TimeoutExpired:
        os.killpg(p.pid, signal.SIGKILL)
        p.wait()
        rc = 'timeout'
el = time.time() - t0
if rc == 'timeout':
    words = 'timed out after %ss (killed)' % limit
elif rc < 0:
    n = -rc
    try:
        name = signal.Signals(n).name
    except ValueError:
        name = '?'
    words = 'sh terminated by signal %d (%s)' % (n, name)
else:
    words = 'sh exited %d' % rc
    if rc > 128:
        try:
            words += ' (128+%d = %s, reported by a shell)' % (rc - 128, signal.Signals(rc - 128).name)
        except ValueError:
            pass
open(os.path.join(d, 'exit'), 'w').write('%s\n' % rc)
open(os.path.join(d, 'status'), 'w').write(words + '\n')
open(os.path.join(d, 'elapsed'), 'w').write('%.1f\n' % el)
vf = os.path.join(d, 'version.cmd')
if os.path.exists(vf):
    vcmd = open(vf).read().rstrip('\n')
    v = subprocess.run(['sh', '-c', vcmd], cwd=d, stdin=subprocess.DEVNULL,
                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    open(os.path.join(d, 'version'), 'wb').write(v.stdout)
print('%-60s exit=%-8s %5.1fs' % (os.path.relpath(d, os.path.dirname(os.path.dirname(d))), rc, el))
