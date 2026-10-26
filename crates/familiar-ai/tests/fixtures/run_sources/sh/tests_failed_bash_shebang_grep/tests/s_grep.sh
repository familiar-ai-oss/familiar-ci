#!/bin/bash
printf 'alpha\nbeta\n' > /tmp/a0-sh-$$.txt
echo checking
grep -q gamma /tmp/a0-sh-$$.txt
