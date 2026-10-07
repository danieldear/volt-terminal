#!/usr/bin/env python3
"""Compare saved cachebench binaries. Offscreen CPU/GPU, NOT display or PTY speed.
Run only after builds and other tests finish. Alternates order after warm-up.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import statistics
import subprocess

CASES = {'ascii-full', 'ascii-one-row', 'unicode-full', 'unicode-static', 'tasks-full'}
METRICS = ('cpu_p50_ms', 'cpu_p95_ms', 'completion_p50_ms', 'completion_p95_ms')


def validate(rows):
    if len(rows) != len(CASES) or {r['workload'] for r in rows} != CASES:
        raise ValueError('missing or duplicate rendering workload')
    for row in rows:
        if row['frames'] != 300:
            raise ValueError('unexpected frame count')
        if any(not math.isfinite(row[key]) or row[key] <= 0 for key in METRICS):
            raise ValueError('invalid rendering duration')
    return rows


def summarize(records):
    result = []
    for case in sorted(CASES):
        selected = {label: [r for r in records if r['build'] == label and r['workload'] == case]
                    for label in ('before', 'after')}
        if not selected['before'] or len(selected['before']) != len(selected['after']):
            raise ValueError('unequal repeat counts')
        row = {'workload': case}
        for key in METRICS:
            values = {label: statistics.median(r[key] for r in rows) for label, rows in selected.items()}
            row[key] = dict(**values, percent_change=100 * (values['after'] / values['before'] - 1))
        result.append(row)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('before', type=Path)
    parser.add_argument('after', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--repeats', type=int, default=5)
    parser.add_argument('--max-regression-percent', type=float, default=5)
    args = parser.parse_args()
    if not 3 <= args.repeats <= 50 or not 0 <= args.max_regression_percent <= 100:
        parser.error('invalid repeat count or budget')
    bins = {label: getattr(args, label).resolve() for label in ('before', 'after')}
    def run(label):
        env = os.environ.copy()
        env.pop('VOLT_PERF', None)
        output = subprocess.check_output([str(bins[label])], text=True, timeout=180, env=env)
        return validate([json.loads(line) for line in output.splitlines()])
    for label in bins:
        run(label)
    records = []
    for repeat in range(args.repeats):
        for label in (('before', 'after') if repeat % 2 == 0 else ('after', 'before')):
            records.extend(dict(row, build=label, repeat=repeat) for row in run(label))
    summary = summarize(records)
    report = dict(metric='offscreen_rendering_not_display_or_pty_throughput',
                  executables={label: dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest()) for label, path in bins.items()},
                  summary=summary, samples=records)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(summary, indent=2))
    if any(row['cpu_p50_ms']['percent_change'] > args.max_regression_percent for row in summary):
        raise SystemExit('CPU rendering regression budget exceeded; investigate')


if __name__ == '__main__':
    main()
