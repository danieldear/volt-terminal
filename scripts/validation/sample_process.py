#!/usr/bin/env python3
"""Sample an explicitly chosen process. CPU/RSS only, NOT battery or power.

Usage: sample_process.py PID --seconds 60 --output target/process-sample.json
No process arguments, environment, terminal output or clipboard are captured.
"""
import argparse
import json
import platform
import subprocess
import time
from pathlib import Path


def cpu_seconds(value):
    days, value = value.split('-', 1) if '-' in value else ('0', value)
    parts = [float(x) for x in value.split(':')]
    result = 0.0
    for part in parts:
        result = result * 60 + part
    return float(days) * 86400 + result


def sample(pid):
    # lstart guards against accidentally sampling a reused PID.
    fields = subprocess.check_output(
        ['ps', '-p', str(pid), '-o', 'time=', '-o', 'rss=', '-o', 'lstart='], text=True
    ).split()
    if len(fields) < 7:
        raise RuntimeError('process is no longer running')
    return dict(cpu_s=cpu_seconds(fields[0]), rss_kib=int(fields[1]), start=' '.join(fields[2:]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pid', type=int)
    parser.add_argument('--seconds', type=float, default=60)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.pid <= 0 or not 1 <= args.seconds <= 3600:
        parser.error('PID must be positive; seconds must be 1..3600')
    initial = sample(args.pid)
    started = time.monotonic()
    rows = [dict(elapsed_s=0, **initial)]
    while time.monotonic() - started < args.seconds:
        time.sleep(min(1, max(0, args.seconds - (time.monotonic() - started))))
        row = sample(args.pid)
        if row['start'] != initial['start']:
            raise RuntimeError('PID was reused; discarding sample')
        rows.append(dict(elapsed_s=time.monotonic() - started, **row))
    elapsed = rows[-1]['elapsed_s']
    cpu = rows[-1]['cpu_s'] - initial['cpu_s']
    result = dict(metric='process_cpu_rss_not_power', platform=platform.platform(),
                  pid=args.pid, elapsed_s=elapsed, cpu_s=cpu,
                  cpu_percent_one_core=100 * cpu / elapsed,
                  rss_min_kib=min(r['rss_kib'] for r in rows),
                  rss_max_kib=max(r['rss_kib'] for r in rows), samples=rows)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: v for k, v in result.items() if k != 'samples'}))


if __name__ == '__main__':
    main()
