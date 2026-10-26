<?php
declare(strict_types=1);
namespace Familiar;

use PHPUnit\Event\Event;
use PHPUnit\Event\Facade as EventFacade;
use PHPUnit\Event\Tracer\Tracer;
use PHPUnit\Runner\Extension\Extension;
use PHPUnit\Runner\Extension\Facade;
use PHPUnit\Runner\Extension\ParameterCollection;
use PHPUnit\TextUI\Configuration\Configuration;

final class PhpunitExtension implements Extension
{
    public function bootstrap(Configuration $configuration, Facade $facade, ParameterCollection $parameters): void
    {
        try { Recorder::install(EventFacade::instance(), 'extension'); } catch (\Throwable $e) { Recorder::kitError($e); }
    }
}

final class Recorder implements Tracer
{
    private static ?Recorder $me = null;
    private string $path;
    private string $root;
    private array $counts = ['passed' => 0, 'failed' => 0, 'skipped' => 0, 'deselected' => 0, 'errors' => 0];
    private array $done = [];
    private bool $ended = false;
    private string $phase = 'bootstrap';

    public static function install(EventFacade $f, string $how): void
    {
        if (self::$me !== null) { return; }
        $r = new self(); self::$me = $r;
        $f->registerTracer($r);
        $r->line(['type' => 'session', 'v' => 1, 'runner' => 'phpunit', 'runner_version' => \PHPUnit\Runner\Version::id(),
            'language' => 'php', 'language_version' => PHP_VERSION, 'plugins' => [], 'loaded_by' => $how]);
        register_shutdown_function([$r, 'shutdown']);
    }

    public static function kitError(\Throwable $e): void
    {
        $p = getenv('FAMILIAR_AI_REPORT');
        if ($p !== false) { @file_put_contents($p, json_encode(['type' => 'kit_error', 'message' => $e->getMessage()]) . "\n", FILE_APPEND | LOCK_EX); }
    }

    private function __construct() { $this->path = (string) getenv('FAMILIAR_AI_REPORT'); $this->root = (string) getcwd(); }

    private function line(array $a): void
    {
        @file_put_contents($this->path, json_encode($a, JSON_UNESCAPED_SLASHES) . "\n", FILE_APPEND | LOCK_EX);
    }

    private function rel(string $f): string
    {
        return str_starts_with($f, $this->root . '/') ? substr($f, strlen($this->root) + 1) : $f;
    }

    /** "file:line" lines of a PHPUnit trace (deepest first) -> frames outermost first, project only */
    private function frames(string $trace, array $extra = []): array
    {
        $out = [];
        foreach (preg_split('/\R/', $trace) as $l) {
            if (preg_match('/^(.+):(\d+)$/', trim($l), $m) && !str_contains($m[1], '/vendor/')) {
                $out[] = ['file' => $this->rel($m[1]), 'line' => (int) $m[2], 'function' => null];
            }
        }
        foreach ($extra as $e) { array_unshift($out, $e); }
        return array_reverse($out);
    }

    private static function kindOf(string $type, string $msg, bool $assertion): string
    {
        if ($assertion) { return 'assertion'; }
        if ($type === 'Error' && preg_match('/^(Call to undefined (method|function)|Class "[^"]+" not found|Undefined constant)/', $msg)) { return 'missing_name'; }
        return 'exception';
    }

    private function test(Event $e, string $outcome, ?array $error = null, ?string $skip = null): void
    {
        $t = $e->test();
        $id = $t->id();
        if (isset($this->done[$id])) { return; }
        $this->done[$id] = 1;
        $file = method_exists($t, 'file') ? $this->rel($t->file()) : null;
        $line = method_exists($t, 'line') ? $t->line() : null;
        $this->line(['type' => 'test', 'id' => $id, 'file' => $file, 'line' => $line, 'outcome' => $outcome, 'skip_reason' => $skip, 'error' => $error]);
    }

    private function err(\PHPUnit\Event\Code\Throwable $th, bool $assertion, ?string $kind = null): array
    {
        return ['type' => $th->className(), 'message' => $th->message(),
            'kind' => $kind ?? self::kindOf($th->className(), $th->message(), $assertion),
            'frames' => $this->frames($th->stackTrace())];
    }

    public function trace(Event $event): void
    {
        try {
            $c = substr(strrchr(get_class($event), '\\'), 1);
            $ns = get_class($event);
            switch (true) {
                case $ns === \PHPUnit\Event\Test\Passed::class:
                    $this->counts['passed']++; $this->test($event, 'passed'); break;
                case $ns === \PHPUnit\Event\Test\Failed::class:
                    $this->counts['failed']++; $this->test($event, 'failed', $this->err($event->throwable(), true)); break;
                case $ns === \PHPUnit\Event\Test\Errored::class:
                    $this->counts['errors']++; $this->test($event, 'error', $this->err($event->throwable(), false)); break;
                case $ns === \PHPUnit\Event\Test\Skipped::class:
                case $ns === \PHPUnit\Event\Test\MarkedIncomplete::class:
                    $this->counts['skipped']++; $this->test($event, 'skipped', null, method_exists($event, 'message') ? $event->message() : null); break;
                case str_contains($ns, '\\Test\\') && (str_ends_with($c, 'MethodErrored') || str_ends_with($c, 'MethodFailed') || $c === 'PreparationErrored' || $c === 'PreparationFailed' || str_starts_with($c, 'PreCondition') && str_ends_with($c, 'Errored')):
                    // a hook (setUp, setUpBeforeClass, ...) or the preparation failed
                    $th = $event->throwable();
                    $this->counts['errors']++;
                    $t = method_exists($event, 'test') ? $event->test() : null;
                    $id = $t ? $t->id() : ($event->testClassName() ?? 'hook');
                    if (!isset($this->done[$id])) {
                        $this->done[$id] = 1;
                        $this->line(['type' => 'test', 'id' => $id, 'file' => $t && method_exists($t, 'file') ? $this->rel($t->file()) : null,
                            'line' => null, 'outcome' => 'error', 'skip_reason' => null, 'error' => $this->err($th, false, 'setup')]);
                    }
                    break;
                case $ns === \PHPUnit\Event\TestRunner\WarningTriggered::class:
                    // e.g. "No tests found in class" -- kept as a note, not an error
                    $this->line(['type' => 'note', 'source' => 'runner_warning', 'message' => $event->message()]); break;
                case $ns === \PHPUnit\Event\TestRunner\EventFacadeSealed::class:
                    $this->phase = 'loading'; break;
                case $ns === \PHPUnit\Event\TestSuite\Loaded::class:
                    $this->phase = 'loaded'; break;
                case $ns === \PHPUnit\Event\TestRunner\ExecutionStarted::class:
                    $this->phase = 'executing'; break;
                case $ns === \PHPUnit\Event\TestRunner\ExecutionFinished::class:
                    $this->end(); break;
            }
        } catch (\Throwable $e) {
            self::kitError($e);
        }
    }

    private function end(): void
    {
        if ($this->ended) { return; }
        $this->ended = true;
        // PHPUnit does not expose its exit code to an extension: exit stays null
        $this->line(['type' => 'end', 'exit' => null, 'counts' => $this->counts]);
    }

    public function shutdown(): void
    {
        try {
            if ($this->ended) { return; }
            $e = error_get_last();
            if ($e !== null && in_array($e['type'], [E_ERROR, E_COMPILE_ERROR, E_CORE_ERROR, E_PARSE, E_USER_ERROR], true)) {
                $missing = (bool) preg_match('/^(Call to undefined|Class "[^"]+" not found)/', $e['message']);
                $this->line(['type' => 'collect_error', 'file' => $this->rel($e['file']), 'error' => ['type' => 'fatal', 'message' => $e['message'],
                    'kind' => $missing ? 'missing_name' : 'exception', 'frames' => [['file' => $this->rel($e['file']), 'line' => $e['line'], 'function' => null]]]]);
                $this->end();
            } elseif ($this->phase === 'loading') {
                // PHPUnit caught a Throwable while loading the test files and exited through its
                // crash message; no event carries it, so the message is not known here.
                $this->line(['type' => 'collect_error', 'file' => null, 'error' => ['type' => 'aborted_while_loading_tests',
                    'message' => null, 'kind' => 'exception', 'frames' => []]]);
                $this->end();
            }
            // any other phase: no end line (the run did not finish; the next reader decides)
        } catch (\Throwable $x) { self::kitError($x); }
    }
}
