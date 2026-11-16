import sys
from pathlib import Path
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare_musicxml_transcript import compare, transcript, TranscriptError


def score(div=1, reverse=False, voice_a="1", voice_b="2"):
    def note(step, voice, chord=False, tie="", grace=False):
        prefix = "<chord/>" if chord else ""
        timing = f'<grace make-time="{div}" steal-time-following="10"/>' if grace else ""
        duration = "" if grace else f"<duration>{div}</duration>"
        endpoint = f'<tie type="{tie}"/>' if tie else ""
        return f'<note>{prefix}{timing}<pitch><step>{step}</step><alter>0</alter><octave>4</octave></pitch>{duration}<voice>{voice}</voice>{endpoint}</note>'

    a = note("D", voice_a, grace=True) + note("C", voice_a, tie="start") + note("E", voice_a, chord=True)
    b = f'<note><rest/><duration>{div}</duration><voice>{voice_b}</voice></note>'
    lanes = [b, a] if reverse else [a, b]
    return f'<score-partwise><part-list><score-part id="p"><part-name>Piano</part-name></score-part></part-list><part id="p"><measure number="1"><attributes><divisions>{div}</divisions></attributes>{lanes[0]}<backup><duration>{div}</duration></backup>{lanes[1]}</measure><measure number="2">{note("C",voice_a,tie="stop")}<forward><duration>{div}</duration></forward></measure></part></score-partwise>'


class TranscriptTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.a = Path(self.temp.name) / "source.xml"
        self.b = Path(self.temp.name) / "export.xml"
        self.a.write_text(score())

    def check_variant(self, source, good=False):
        self.b.write_text(source)
        if good:
            self.assertTrue(compare(self.a, self.b)["equal"])
        else:
            with self.assertRaises(TranscriptError):
                compare(self.a, self.b)

    def test_divisions_lane_order_and_voice_renaming_normalize(self):
        self.check_variant(score(4, True, "upper", "lower"), True)

    def test_pitch_duration_tie_and_grace_corruption_detected(self):
        for bad in [score().replace('<step>E</step>', '<step>F</step>'), score().replace('<duration>1</duration>', '<duration>2</duration>', 1), score().replace('<tie type="stop"/>',''), score().replace('steal-time-following="10"','steal-time-following="11"'), score().replace('make-time="1"','make-time="2"'), score().replace('<chord/>','')]:
            with self.subTest(bad=bad):
                self.check_variant(bad)

    def test_actual_alter_not_accidental_glyph(self):
        self.check_variant(score().replace('<alter>0</alter>', '<alter>0</alter><accidental>natural</accidental>'), True)
        self.check_variant(score().replace('<alter>0</alter>', '<alter>1</alter>', 1))

    def test_forward_extent_is_not_lost(self):
        self.check_variant(score().replace('<forward><duration>1</duration>', '<forward><duration>2</duration>'))

    def test_unattached_grace_and_chord_fail_contextually(self):
        self.b.write_text(score().replace('<note><pitch><step>C</step>', '<note><chord/><pitch><step>C</step>', 1))
        with self.assertRaisesRegex(TranscriptError, 'part 1, measure 1'):
            transcript(self.b)
        dangling = '<note><grace/><pitch><step>D</step><octave>4</octave></pitch><voice>1</voice></note>'
        self.b.write_text(score().replace('</measure></part>', dangling + '</measure></part>'))
        with self.assertRaisesRegex(TranscriptError, 'unattached grace'):
            transcript(self.b)

    def test_identical_voice_lanes_are_explicitly_ambiguous(self):
        source = '<score-partwise><part-list><score-part id="p"/></part-list><part id="p"><measure number="1"><attributes><divisions>1</divisions></attributes><note><rest/><duration>1</duration><voice>1</voice></note><backup><duration>1</duration></backup><note><rest/><duration>1</duration><voice>2</voice></note></measure></part></score-partwise>'
        self.b.write_text(source)
        with self.assertRaisesRegex(TranscriptError, 'identity matching ambiguous'):
            transcript(self.b)

    def test_transposition_changes_are_detected(self):
        self.check_variant(score().replace('</divisions>', '</divisions><transpose><chromatic>2</chromatic></transpose>'))


if __name__ == "__main__":
    unittest.main()
