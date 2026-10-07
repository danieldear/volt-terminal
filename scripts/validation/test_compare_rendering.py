import unittest
import compare_rendering as bench

class RenderingComparisonTests(unittest.TestCase):
    def rows(self, value=1):
        return [dict(workload=case, frames=300, **{key: value for key in bench.METRICS}) for case in bench.CASES]
    def test_workloads_and_durations_are_validated(self):
        for bad in [self.rows()[:-1], self.rows(0), self.rows(float('nan'))]:
            with self.assertRaises(ValueError): bench.validate(bad)
        bad = self.rows(); bad[0]['frames'] = 299
        with self.assertRaises(ValueError): bench.validate(bad)
    def test_matched_medians(self):
        records = []
        for value in [1, 3, 2]:
            records.extend(dict(row, build='before') for row in self.rows(value))
            records.extend(dict(row, build='after') for row in self.rows(value / 2))
        for row in bench.summarize(records):
            self.assertEqual(row['cpu_p50_ms'], dict(before=2, after=1, percent_change=-50))
    def test_missing_repeat_rejected(self):
        rows = [dict(row, build='before') for row in self.rows()]
        with self.assertRaises(ValueError): bench.summarize(rows)

if __name__ == '__main__': unittest.main()
