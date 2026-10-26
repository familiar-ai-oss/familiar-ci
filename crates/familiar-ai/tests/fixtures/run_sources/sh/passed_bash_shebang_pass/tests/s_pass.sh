#!/bin/bash
out=$(printf 'hello\n')
[ "$out" = hello ]
echo "$out" | grep -q hello
echo ok
