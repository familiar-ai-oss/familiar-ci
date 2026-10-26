<?php
// Familiar's prepend: does nothing unless FAMILIAR_AI_REPORT is set. Never prints, never throws.
// It only registers an autoloader for the Familiar\ namespace (the extension in ext.php, which
// PHPUnit loads through the added --extension flag); it never touches the person's own autoloader.
if (getenv('FAMILIAR_AI_REPORT') === false || defined('FAMILIAR_AI_PREPENDED')) { return; }
define('FAMILIAR_AI_PREPENDED', 1);
spl_autoload_register(static function (string $c): void {
    if (strncmp($c, 'Familiar\\', 9) === 0) { require_once __DIR__ . '/ext.php'; }
}, true, true);
