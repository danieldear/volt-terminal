#!/usr/bin/env python3
"""Compare two saved throughputbench executables, alternating order after warmup.

Parser only: this is NOT a PTY/GPU/presentation or whole-app TermMark benchmark.
Run on a quiet machine, after builds and GPU checks finish. A nonzero exit means
at least one case regressed beyond the budget or the workloads did not match.
"""
import argparse
import hashlib
import json
import math
import statistics
import subprocess
from pathlib import Path


def summarize(records):
    keys = {(r['case'], r['wide_seed'], r['cols'], r['rows'], r['bytes']) for r in records}
    result = []
    for case, wide, cols, rows, size in sorted(keys):
        samples = {}
        for label in ('before', 'after'):
            values = [r['mib_s'] for r in records if
                      (r['case'], r['wide_seed'], r['cols'], r['rows'], r['bytes']) ==
                      (case, wide, cols, rows, size) and r['build'] == label]
            if not values or any(not math.isfinite(v) or v <= 0 for v in values):
                raise ValueError('missing or invalid comparable workload')
            samples[label] = values
        if len(samples['before']) != len(samples['after']):
            raise ValueError('unequal repeat counts')
        rates = {k: statistics.median(v) for k, v in samples.items()}
        result.append(dict(case=case, wide_seed=wide, cols=cols, rows=rows, bytes=size,
                           **rates, percent_change=100 * (rates['after'] / rates['before'] - 1)))
    if len(result) != 8:
        raise ValueError('expected eight throughputbench cases')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('before', type=Path)
    parser.add_argument('after', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--repeats', type=int, default=7)
    parser.add_argument('--mib', type=int, default=256)
    parser.add_argument('--cols', type=int, default=91)
    parser.add_argument('--rows', type=int, default=16)
    parser.add_argument('--max-regression-percent', type=float, default=5)
    args = parser.parse_args()
    if not (3 <= args.repeats <= 50 and 1 <= args.mib <= 4096
            and 1 <= args.cols <= 1000 and 1 <= args.rows <= 1000
            and 0 <= args.max_regression_percent <= 100):
        parser.error('invalid workload, repeat count or regression budget')
    bins = {k: getattr(args, k).resolve() for k in ('before', 'after')}
    records = []

    def run(label):
        output = subprocess.check_output(
            [str(bins[label]), str(args.mib), str(args.cols), str(args.rows)],
            text=True, timeout=300)
        return [dict(json.loads(line), build=label) for line in output.splitlines()]

    for label in bins:
        run(label)
    for repeat in range(args.repeats):
        for label in (('before', 'after') if repeat % 2 == 0 else ('after', 'before')):
            records.extend(dict(r, repeat=repeat) for r in run(label))
    result = summarize(records)
    report = dict(metric='parser_throughput_not_whole_app',
                  executables={k: dict(path=str(p), sha256=hashlib.sha256(p.read_bytes()).hexdigest())
                               for k, p in bins.items()},
                  summary=result, samples=records)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(result, indent=2))
    if any(r['percent_change'] < -args.max_regression_percent for r in result):
        raise SystemExit('throughput regression budget exceeded; investigate before shipping')


if __name__ == '__main__':
    main()
