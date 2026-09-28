import unittest
from compare_throughput import summarize


def data():
    return [dict(case=case, wide_seed=wide, cols=91, rows=16, bytes=1024,
                 mib_s=100 if label == 'before' else 96, build=label)
            for case in ('many', 'long', 'fg', 'fgbg') for wide in (False, True)
            for label in ('before', 'after') for _ in range(3)]


class CompareThroughputTests(unittest.TestCase):
    def test_medians_and_regression_sign(self):
        rows = summarize(data())
        self.assertEqual(len(rows), 8)
        self.assertTrue(all(abs(r['percent_change'] + 4) < 1e-9 for r in rows))

    def test_invalid_measurements_fail_closed(self):
        for rate in (0, -1, float('nan'), float('inf')):
            rows = data()
            rows[0]['mib_s'] = rate
            with self.assertRaises(ValueError):
                summarize(rows)

    def test_missing_or_different_workloads_fail(self):
        rows = data()
        rows[0]['bytes'] = 2048
        for invalid in (rows, data()[1:], [], data()[:6]):
            with self.assertRaises(ValueError):
                summarize(invalid)


if __name__ == '__main__':
    unittest.main()
