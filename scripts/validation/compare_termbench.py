#!/usr/bin/env python3
"""Fail-closed comparison of matched visible-window TermMark trials.

Input JSON: context contains grid/font/scale/scrollback/hooks/workspace settings;
build contains commit/binary_sha256/config_sha256; trials contains >=3 records
with ManyLine, LongLine, FGPerChar, FGBGPerChar, TermMarkV2 seconds. No terminal
contents, environment, passwords or configuration values are collected.
This validates supplied measurements, not whether the terminal rendered them.
"""
import argparse
import json
import math
import statistics
from pathlib import Path

CASES = ('ManyLine', 'LongLine', 'FGPerChar', 'FGBGPerChar', 'TermMarkV2')
CONTEXT = {'cols', 'rows', 'font', 'font_size', 'line_height', 'scale',
           'scrollback', 'shell_integration', 'workspace'}


def validate(report):
    context = report['context']
    if set(context) != CONTEXT or context['workspace'] not in ('closed', 'floating', 'docked'):
        raise ValueError('complete workload context is required')
    for key in ('cols', 'rows', 'font_size', 'line_height', 'scale', 'scrollback'):
        n = context[key]
        if isinstance(n, bool) or not isinstance(n, (int, float)) or not math.isfinite(n) or n <= 0:
            raise ValueError('invalid workload context')
    if not isinstance(context['shell_integration'], bool) or not isinstance(context['font'], str) or not context['font']:
        raise ValueError('invalid font/hooks context')
    for field, length in (('commit', 40), ('binary_sha256', 64), ('config_sha256', 64)):
        s = report['build'][field]
        if not isinstance(s, str) or len(s) != length or any(c not in '0123456789abcdef' for c in s):
            raise ValueError('exact build/config identity is required')
    trials = report['trials']
    if not isinstance(trials, list) or not 3 <= len(trials) <= 50:
        raise ValueError('at least three trials per build are required')
    for trial in trials:
        if set(trial) != set(CASES):
            raise ValueError('missing or duplicate workload')
        if any(isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v) or v <= 0 for v in trial.values()):
            raise ValueError('invalid measurement')
    return report


def compare(before, after):
    before, after = validate(before), validate(after)
    if before['context'] != after['context'] or len(before['trials']) != len(after['trials']):
        raise ValueError('comparison must use matched context and equal trial counts')
    result = []
    for case in CASES:
        samples = {k: [trial[case] for trial in r['trials']] for k, r in (('before', before), ('after', after))}
        medians = {k: statistics.median(v) for k, v in samples.items()}
        result.append(dict(case=case, **medians, percent_change=100 * (medians['after'] / medians['before'] - 1),
                           ranges={k: [min(v), max(v)] for k, v in samples.items()}))
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('before', type=Path)
    p.add_argument('after', type=Path)
    p.add_argument('--max-regression-percent', type=float, default=5)
    args = p.parse_args()
    if not math.isfinite(args.max_regression_percent) or not 0 <= args.max_regression_percent <= 100:
        p.error('invalid regression budget')
    result = compare(json.loads(args.before.read_text()), json.loads(args.after.read_text()))
    print(json.dumps(result, indent=2))
    if any(r['percent_change'] > args.max_regression_percent for r in result):
        raise SystemExit('regression budget exceeded; do not ship')


if __name__ == '__main__':
    main()
