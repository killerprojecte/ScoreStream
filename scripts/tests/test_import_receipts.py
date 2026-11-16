import importlib.util
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from check_beethoven_import import run
class ReceiptTests(unittest.TestCase):
 def test_counts_and_fingerprint_keep_aggregate_occurrences(self):
  d=dict(effect='preserved',feature='retention',path='/first',occurrences=2048)
  with patch('check_beethoven_import.subprocess.run') as launch:
   launch.return_value.returncode=0;launch.return_value.stdout=b'score';launch.return_value.stderr=json.dumps({'diagnostics':[d,dict(effect='normalized',feature='identity',path='/')]}).encode()
   report,_,fingerprint=run('cli','score')
   self.assertEqual(report['diagnostics'][0]['reported_count'],1)
   self.assertEqual(report['diagnostics'][0]['occurrence_count'],2048)
   self.assertEqual(fingerprint[0][-1],2048)
   self.assertEqual(fingerprint[1][-1],1)
if __name__=='__main__':unittest.main()
