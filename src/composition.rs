#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Slot {
    Initial,
    Medial,
    Final,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Jamo {
    Initial(char),
    Medial(char),
    Final(char),
}

impl Jamo {
    fn slot(self) -> Slot {
        match self {
            Self::Initial(_) => Slot::Initial,
            Self::Medial(_) => Slot::Medial,
            Self::Final(_) => Slot::Final,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JamoInput {
    pub jamo: Jamo,
    pub source_key: char,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Input {
    Jamo(JamoInput),
    Emit(char),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Transition {
    pub committed: String,
    pub preedit: String,
}

#[derive(Clone, Debug, Default)]
pub struct Composer {
    stack: Vec<JamoInput>,
}

#[derive(Clone, Debug, Default)]
struct Parts {
    initial: Option<char>,
    medial: Option<char>,
    final_: Option<char>,
    order: Vec<Slot>,
}

impl Composer {
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn preedit(&self) -> String {
        visible(&self.stack)
    }

    pub fn push(&mut self, input: JamoInput) -> Transition {
        let committed = if insertion_starts_new_composition(&self.stack, input) {
            self.flush()
        } else {
            String::new()
        };

        self.stack.push(input);
        Transition {
            committed,
            preedit: self.preedit(),
        }
    }

    pub fn backspace(&mut self) -> bool {
        self.stack.pop().is_some()
    }

    pub fn flush(&mut self) -> String {
        let text = self.preedit();
        self.stack.clear();
        text
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }
}

fn insertion_starts_new_composition(stack: &[JamoInput], next: JamoInput) -> bool {
    if stack.is_empty() {
        return false;
    }

    let parts = reduce(stack);
    match next.jamo.slot() {
        Slot::Initial => {
            parts.medial.is_some()
                || parts.final_.is_some()
                || (parts.initial.is_some() && combined_initial(stack, next).is_none())
        }
        Slot::Medial => {
            let standalone_final = parts.initial.is_none() && parts.final_.is_some();
            let complete_syllable = parts.medial.is_some() && parts.final_.is_some();
            let uncombinable_medial =
                parts.medial.is_some() && combined_medial(stack, next).is_none();
            standalone_final || complete_syllable || uncombinable_medial
        }
        Slot::Final => {
            let standalone_medial = parts.initial.is_none() && parts.medial.is_some();
            let uncombinable_final =
                parts.final_.is_some() && combined_final(stack, next).is_none();
            standalone_medial || uncombinable_final
        }
    }
}

fn reduce(stack: &[JamoInput]) -> Parts {
    let mut parts = Parts::default();
    for input in stack {
        let slot = input.jamo.slot();
        if !parts.order.contains(&slot) {
            parts.order.push(slot);
        }

        match input.jamo {
            Jamo::Initial(value) => {
                parts.initial = Some(match parts.initial {
                    None => value,
                    Some(first) => combine_initial(first, value)
                        .expect("active composition contains combinable initials"),
                });
            }
            Jamo::Medial(value) => {
                parts.medial = Some(match parts.medial {
                    None => value,
                    Some(first) => combine_medial(first, value, stack)
                        .expect("active composition contains combinable medials"),
                });
            }
            Jamo::Final(value) => {
                parts.final_ = Some(match parts.final_ {
                    None => value,
                    Some(first) => combine_final(first, value)
                        .expect("active composition contains combinable finals"),
                });
            }
        }
    }
    parts
}

fn combined_initial(stack: &[JamoInput], next: JamoInput) -> Option<char> {
    let Jamo::Initial(next) = next.jamo else {
        return None;
    };
    combine_initial(reduce(stack).initial?, next)
}

fn combined_medial(stack: &[JamoInput], next: JamoInput) -> Option<char> {
    let Jamo::Medial(next_value) = next.jamo else {
        return None;
    };
    let existing = stack
        .iter()
        .filter(|input| matches!(input.jamo, Jamo::Medial(_)))
        .collect::<Vec<_>>();
    if existing.len() != 1 {
        return None;
    }
    let Jamo::Medial(existing_value) = existing[0].jamo else {
        unreachable!();
    };
    combine_medial_pair(
        existing_value,
        next_value,
        existing[0].source_key,
        next.source_key,
    )
}

fn combined_final(stack: &[JamoInput], next: JamoInput) -> Option<char> {
    let Jamo::Final(next) = next.jamo else {
        return None;
    };
    combine_final(reduce(stack).final_?, next)
}

fn visible(stack: &[JamoInput]) -> String {
    if stack.is_empty() {
        return String::new();
    }

    let parts = reduce(stack);
    if let (Some(initial), Some(medial)) = (parts.initial, parts.medial)
        && let Some(syllable) = compose_syllable(initial, medial, parts.final_)
    {
        return syllable.to_string();
    }

    parts
        .order
        .iter()
        .filter_map(|slot| match slot {
            Slot::Initial => parts.initial,
            Slot::Medial => parts.medial,
            Slot::Final => parts.final_,
        })
        .collect()
}

fn unordered_pair(a: char, b: char, left: char, right: char) -> bool {
    (a == left && b == right) || (a == right && b == left)
}

fn combine_initial(a: char, b: char) -> Option<char> {
    [
        ('ㅇ', 'ㄱ', 'ㄲ'),
        ('ㄱ', 'ㅈ', 'ㅉ'),
        ('ㅈ', 'ㅂ', 'ㅃ'),
        ('ㄷ', 'ㅁ', 'ㄸ'),
        ('ㅅ', 'ㅎ', 'ㅆ'),
    ]
    .into_iter()
    .find_map(|(left, right, result)| unordered_pair(a, b, left, right).then_some(result))
}

fn combine_medial(a: char, b: char, stack: &[JamoInput]) -> Option<char> {
    let medials = stack
        .iter()
        .filter(|input| matches!(input.jamo, Jamo::Medial(_)))
        .collect::<Vec<_>>();
    if medials.len() != 2 {
        return None;
    }
    combine_medial_pair(a, b, medials[0].source_key, medials[1].source_key)
}

fn combine_medial_pair(
    older_value: char,
    newer_value: char,
    older_source: char,
    newer_source: char,
) -> Option<char> {
    let source_specific = match (older_source, newer_source, older_value, newer_value) {
        ('/', _, 'ㅗ', 'ㅏ') | (_, '/', 'ㅏ', 'ㅗ') | ('v', _, 'ㅗ', 'ㅏ') => Some('ㅘ'),
        ('/', _, 'ㅗ', 'ㅐ') | (_, '/', 'ㅐ', 'ㅗ') | ('v', _, 'ㅗ', 'ㅐ') => Some('ㅙ'),
        ('/', _, 'ㅗ', 'ㅣ') | (_, '/', 'ㅣ', 'ㅗ') | ('v', _, 'ㅗ', 'ㅣ') => Some('ㅚ'),
        ('9', _, 'ㅜ', 'ㅓ') | (_, '9', 'ㅓ', 'ㅜ') | ('b', _, 'ㅜ', 'ㅓ') => Some('ㅝ'),
        ('9', _, 'ㅜ', 'ㅔ') | (_, '9', 'ㅔ', 'ㅜ') | ('b', _, 'ㅜ', 'ㅔ') => Some('ㅞ'),
        ('9', _, 'ㅜ', 'ㅣ') | (_, '9', 'ㅣ', 'ㅜ') | ('b', _, 'ㅜ', 'ㅣ') => Some('ㅟ'),
        _ => None,
    };
    if source_specific.is_some() {
        return source_specific;
    }

    if matches!(older_source, '/' | '9' | 'v' | 'b')
        || matches!(newer_source, '/' | '9' | 'v' | 'b')
    {
        return None;
    }

    [('ㅣ', 'ㅏ', 'ㅒ'), ('ㅣ', 'ㅡ', 'ㅢ'), ('ㅕ', 'ㅐ', 'ㅖ')]
        .into_iter()
        .find_map(|(left, right, result)| {
            unordered_pair(older_value, newer_value, left, right).then_some(result)
        })
}

fn combine_final(a: char, b: char) -> Option<char> {
    for (left, right, result) in [
        ('ㅇ', 'ㄴ', 'ㄶ'),
        ('ㅅ', 'ㄹ', 'ㅄ'),
        ('ㅂ', 'ㅆ', 'ㅈ'),
        ('ㅎ', 'ㅆ', 'ㄲ'),
    ] {
        if unordered_pair(a, b, left, right) {
            return Some(result);
        }
    }

    match (a, b) {
        ('ㄱ', 'ㄱ') => Some('ㄲ'),
        ('ㄱ', 'ㅅ') => Some('ㄳ'),
        ('ㄴ', 'ㅈ') => Some('ㄵ'),
        ('ㄴ', 'ㅎ') => Some('ㄶ'),
        ('ㄹ', 'ㄱ') => Some('ㄺ'),
        ('ㄹ', 'ㅁ') => Some('ㄻ'),
        ('ㄹ', 'ㅂ') => Some('ㄼ'),
        ('ㄹ', 'ㅌ') => Some('ㄾ'),
        ('ㄹ', 'ㅍ') => Some('ㄿ'),
        ('ㄹ', 'ㅎ') => Some('ㅀ'),
        ('ㅂ', 'ㅅ') => Some('ㅄ'),
        ('ㅅ', 'ㅅ') => Some('ㅆ'),
        _ => None,
    }
}

fn compose_syllable(initial: char, medial: char, final_: Option<char>) -> Option<char> {
    const INITIALS: &str = "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ";
    const MEDIALS: &str = "ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ";
    const FINALS: &str = " ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ";
    let initial_index = INITIALS.chars().position(|value| value == initial)? as u32;
    let medial_index = MEDIALS.chars().position(|value| value == medial)? as u32;
    let final_index = match final_ {
        Some(target) => FINALS.chars().position(|value| value == target)? as u32,
        None => 0,
    };
    char::from_u32(0xac00 + (initial_index * 21 + medial_index) * 28 + final_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(name: char) -> JamoInput {
        let jamo = match name {
            'k' => Jamo::Initial('ㄱ'),
            'j' => Jamo::Initial('ㅇ'),
            'l' => Jamo::Initial('ㅈ'),
            ';' => Jamo::Initial('ㅂ'),
            'u' => Jamo::Initial('ㄷ'),
            'i' => Jamo::Initial('ㅁ'),
            'n' => Jamo::Initial('ㅅ'),
            'm' => Jamo::Initial('ㅎ'),
            'e' => Jamo::Medial('ㅕ'),
            'f' => Jamo::Medial('ㅏ'),
            'r' => Jamo::Medial('ㅐ'),
            'd' => Jamo::Medial('ㅣ'),
            'g' => Jamo::Medial('ㅡ'),
            't' => Jamo::Medial('ㅓ'),
            'c' => Jamo::Medial('ㅔ'),
            '/' => Jamo::Medial('ㅗ'),
            '9' => Jamo::Medial('ㅜ'),
            'v' => Jamo::Medial('ㅗ'),
            'b' => Jamo::Medial('ㅜ'),
            'x' => Jamo::Final('ㄱ'),
            'z' => Jamo::Final('ㅁ'),
            'q' => Jamo::Final('ㅅ'),
            'w' => Jamo::Final('ㄹ'),
            'a' => Jamo::Final('ㅇ'),
            's' => Jamo::Final('ㄴ'),
            '1' => Jamo::Final('ㅎ'),
            '2' => Jamo::Final('ㅆ'),
            '3' => Jamo::Final('ㅂ'),
            _ => panic!("unknown test key: {name}"),
        };
        JamoInput {
            jamo,
            source_key: name,
        }
    }

    fn compose(sequence: &str) -> String {
        let mut composer = Composer::default();
        let mut output = String::new();
        for name in sequence.chars().filter(|value| !value.is_whitespace()) {
            output.push_str(&composer.push(key(name)).committed);
        }
        output.push_str(&composer.flush());
        output
    }

    #[test]
    fn initial_first_chording() {
        for (sequence, expected) in [
            ("kfx", "각"),
            ("kxf", "각"),
            ("xkf", "ㄱ가"),
            ("fkx", "ㅏㄱㄱ"),
            ("kfr", "가ㅐ"),
            ("kfxz", "각ㅁ"),
            ("kxqf", "갃"),
            ("kxz", "ㄱㄱㅁ"),
            ("fr", "ㅏㅐ"),
            ("fx", "ㅏㄱ"),
            ("xf", "ㄱㅏ"),
            ("kfxf", "각ㅏ"),
        ] {
            assert_eq!(compose(sequence), expected, "{sequence}");
        }
    }

    #[test]
    fn weak_combinations() {
        for (sequence, expected) in [
            ("jkf", "까"),
            ("kjf", "까"),
            ("jfk", "아ㄱ"),
            ("kkf", "ㄱ가"),
            ("klf", "짜"),
            ("llf", "ㅈ자"),
            ("l;f", "빠"),
            ("uif", "따"),
            ("nmf", "싸"),
            ("df", "ㅒ"),
            ("dg", "ㅢ"),
            ("ker", "계"),
            ("kfas", "갆"),
            ("kfqw", "값"),
            ("kf32", "갖"),
            ("kf12", "갂"),
        ] {
            assert_eq!(compose(sequence), expected, "{sequence}");
        }
    }

    #[test]
    fn source_sensitive_medials() {
        for (sequence, expected) in [
            ("/f", "ㅘ"),
            ("f/", "ㅘ"),
            ("vf", "ㅘ"),
            ("fv", "ㅏㅗ"),
            ("/r", "ㅙ"),
            ("r/", "ㅙ"),
            ("vr", "ㅙ"),
            ("rv", "ㅐㅗ"),
            ("/d", "ㅚ"),
            ("d/", "ㅚ"),
            ("vd", "ㅚ"),
            ("dv", "ㅣㅗ"),
            ("9t", "ㅝ"),
            ("t9", "ㅝ"),
            ("bt", "ㅝ"),
            ("tb", "ㅓㅜ"),
            ("9c", "ㅞ"),
            ("c9", "ㅞ"),
            ("bc", "ㅞ"),
            ("cb", "ㅔㅜ"),
            ("9d", "ㅟ"),
            ("d9", "ㅟ"),
            ("bd", "ㅟ"),
            ("db", "ㅣㅜ"),
        ] {
            assert_eq!(compose(sequence), expected, "{sequence}");
        }
    }

    #[test]
    fn preedit_uses_reduced_compatibility_jamo() {
        let mut composer = Composer::default();
        composer.push(key('/'));
        assert_eq!(composer.preedit(), "ㅗ");
        composer.push(key('f'));
        assert_eq!(composer.preedit(), "ㅘ");

        composer.clear();
        composer.push(key('j'));
        composer.push(key('k'));
        assert_eq!(composer.preedit(), "ㄲ");
    }

    #[test]
    fn backspace_recomputes_from_source_keys() {
        let mut composer = Composer::default();
        composer.push(key('k'));
        composer.push(key('f'));
        assert_eq!(composer.preedit(), "가");
        assert!(composer.backspace());
        assert_eq!(composer.preedit(), "ㄱ");

        composer.push(key('x'));
        assert_eq!(composer.preedit(), "ㄱㄱ");
        assert!(composer.backspace());
        composer.push(key('f'));
        assert_eq!(composer.flush(), "가");
    }

    #[test]
    fn all_modern_syllables_are_contiguous_unicode() {
        let initials = "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ";
        let medials = "ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ";
        let finals = " ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ";
        let mut expected = 0xac00;
        for initial in initials.chars() {
            for medial in medials.chars() {
                for final_ in finals.chars() {
                    let composed =
                        compose_syllable(initial, medial, (final_ != ' ').then_some(final_))
                            .expect("modern jamo must compose");
                    assert_eq!(composed as u32, expected);
                    expected += 1;
                }
            }
        }
        assert_eq!(expected - 1, 0xd7a3);
    }

    #[test]
    fn pair_combination_tables_are_closed_over_each_slot() {
        let initial_expected = [
            ('ㅇ', 'ㄱ', 'ㄲ'),
            ('ㄱ', 'ㅇ', 'ㄲ'),
            ('ㄱ', 'ㅈ', 'ㅉ'),
            ('ㅈ', 'ㄱ', 'ㅉ'),
            ('ㅈ', 'ㅂ', 'ㅃ'),
            ('ㅂ', 'ㅈ', 'ㅃ'),
            ('ㄷ', 'ㅁ', 'ㄸ'),
            ('ㅁ', 'ㄷ', 'ㄸ'),
            ('ㅅ', 'ㅎ', 'ㅆ'),
            ('ㅎ', 'ㅅ', 'ㅆ'),
        ];
        for first in "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ".chars() {
            for second in "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ".chars() {
                let expected = initial_expected
                    .iter()
                    .find_map(|entry| (entry.0 == first && entry.1 == second).then_some(entry.2));
                assert_eq!(combine_initial(first, second), expected);
            }
        }

        let medial_expected = [
            ('ㅣ', 'ㅏ', 'ㅒ'),
            ('ㅏ', 'ㅣ', 'ㅒ'),
            ('ㅣ', 'ㅡ', 'ㅢ'),
            ('ㅡ', 'ㅣ', 'ㅢ'),
            ('ㅕ', 'ㅐ', 'ㅖ'),
            ('ㅐ', 'ㅕ', 'ㅖ'),
        ];
        for first in "ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ".chars() {
            for second in "ㅏㅐㅑㅒㅓㅔㅕㅖㅗㅘㅙㅚㅛㅜㅝㅞㅟㅠㅡㅢㅣ".chars()
            {
                let expected = medial_expected
                    .iter()
                    .find_map(|entry| (entry.0 == first && entry.1 == second).then_some(entry.2));
                assert_eq!(combine_medial_pair(first, second, 'q', 'w'), expected);
            }
        }

        let final_expected = [
            ('ㅇ', 'ㄴ', 'ㄶ'),
            ('ㄴ', 'ㅇ', 'ㄶ'),
            ('ㅅ', 'ㄹ', 'ㅄ'),
            ('ㄹ', 'ㅅ', 'ㅄ'),
            ('ㅂ', 'ㅆ', 'ㅈ'),
            ('ㅆ', 'ㅂ', 'ㅈ'),
            ('ㅎ', 'ㅆ', 'ㄲ'),
            ('ㅆ', 'ㅎ', 'ㄲ'),
            ('ㄱ', 'ㄱ', 'ㄲ'),
            ('ㄱ', 'ㅅ', 'ㄳ'),
            ('ㄴ', 'ㅈ', 'ㄵ'),
            ('ㄴ', 'ㅎ', 'ㄶ'),
            ('ㄹ', 'ㄱ', 'ㄺ'),
            ('ㄹ', 'ㅁ', 'ㄻ'),
            ('ㄹ', 'ㅂ', 'ㄼ'),
            ('ㄹ', 'ㅌ', 'ㄾ'),
            ('ㄹ', 'ㅍ', 'ㄿ'),
            ('ㄹ', 'ㅎ', 'ㅀ'),
            ('ㅂ', 'ㅅ', 'ㅄ'),
            ('ㅅ', 'ㅅ', 'ㅆ'),
        ];
        for first in "ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ".chars()
        {
            for second in "ㄱㄲㄳㄴㄵㄶㄷㄹㄺㄻㄼㄽㄾㄿㅀㅁㅂㅄㅅㅆㅇㅈㅊㅋㅌㅍㅎ".chars()
            {
                let expected = final_expected
                    .iter()
                    .find_map(|entry| (entry.0 == first && entry.1 == second).then_some(entry.2));
                assert_eq!(combine_final(first, second), expected);
            }
        }
    }

    #[test]
    fn jamo_variant_preserves_slot_identity() {
        assert_ne!(Jamo::Initial('ㄱ'), Jamo::Final('ㄱ'));
    }
}
