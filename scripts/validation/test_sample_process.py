"""Unit tests for the opt-in idle CPU regression gate (no real process sampling)."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import sample_process


class CpuBudgetTests(unittest.TestCase):
    def run_sample(self, budget):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / 'sample.json'
            argv = ['sample_process.py', '123', '--seconds', '1',
                    '--output', str(output), '--max-cpu-percent', str(budget)]
            with patch('sys.argv', argv), \
                 patch.object(sample_process, 'sample', side_effect=[
                     dict(cpu_s=0, rss_kib=100, start='same'),
                     dict(cpu_s=1, rss_kib=100, start='same')]), \
                 patch.object(sample_process.time, 'monotonic', side_effect=[0, 0, 0, 1, 1]), \
                 patch.object(sample_process.time, 'sleep'), \
                 contextlib.redirect_stdout(io.StringIO()):
                failure = None
                try:
                    sample_process.main()
                except SystemExit as error:
                    failure = error
            # Even a failed gate must preserve the measurement for diagnosis.
            self.assertEqual(json.loads(output.read_text())['cpu_percent_one_core'], 100)
            return failure

    def test_budget_failure_preserves_report(self):
        self.assertIn('CPU budget exceeded', str(self.run_sample(5)))

    def test_cpu_at_budget_passes(self):
        self.assertIsNone(self.run_sample(100))


if __name__ == '__main__':
    unittest.main()
