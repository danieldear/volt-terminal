import copy
import unittest
from compare_termbench import CASES, compare


def report():
    return dict(context=dict(cols=91, rows=24, font='SF Mono', font_size=14,
                             line_height=1.4, scale=2, scrollback=10000,
                             shell_integration=True, workspace='closed'),
                build=dict(commit='a'*40, binary_sha256='b'*64, config_sha256='c'*64),
                trials=[dict.fromkeys(CASES, 10) for _ in range(3)])


class TermbenchTests(unittest.TestCase):
    def test_medians_keep_each_workload_not_only_total(self):
        a, b = report(), report()
        for t in b['trials']:
            t['FGPerChar'] = 11
            t['TermMarkV2'] = 9
        rows = compare(a, b)
        self.assertAlmostEqual(rows[2]['percent_change'], 10)
        self.assertAlmostEqual(rows[4]['percent_change'], -10)

    def test_unmatched_settings_or_missing_identity_are_rejected(self):
        for field in report()['context']:
            b = copy.deepcopy(report())
            b['context'].pop(field)
            with self.assertRaises((ValueError, KeyError)):
                compare(report(), b)
        b = report(); b['context']['rows'] = 30
        with self.assertRaises(ValueError):
            compare(report(), b)
        b = report(); b['build']['commit'] = 'unknown'
        with self.assertRaises(ValueError):
            compare(report(), b)

    def test_sparse_or_nonfinite_results_are_rejected(self):
        for n in (0, -1, float('nan'), float('inf'), True):
            b = report(); b['trials'][0]['ManyLine'] = n
            with self.assertRaises(ValueError):
                compare(report(), b)
        b = report(); b['trials'].pop()
        with self.assertRaises(ValueError):
            compare(report(), b)
