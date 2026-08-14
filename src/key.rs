use crate::composition::{Input, Jamo, JamoInput};

const QWERTY_CODES: &[(u32, char)] = &[
    (41, '`'),
    (2, '1'),
    (3, '2'),
    (4, '3'),
    (5, '4'),
    (6, '5'),
    (7, '6'),
    (8, '7'),
    (9, '8'),
    (10, '9'),
    (11, '0'),
    (12, '-'),
    (13, '='),
    (16, 'q'),
    (17, 'w'),
    (18, 'e'),
    (19, 'r'),
    (20, 't'),
    (21, 'y'),
    (22, 'u'),
    (23, 'i'),
    (24, 'o'),
    (25, 'p'),
    (26, '['),
    (27, ']'),
    (43, '\\'),
    (30, 'a'),
    (31, 's'),
    (32, 'd'),
    (33, 'f'),
    (34, 'g'),
    (35, 'h'),
    (36, 'j'),
    (37, 'k'),
    (38, 'l'),
    (39, ';'),
    (40, '\''),
    (44, 'z'),
    (45, 'x'),
    (46, 'c'),
    (47, 'v'),
    (48, 'b'),
    (49, 'n'),
    (50, 'm'),
    (51, ','),
    (52, '.'),
    (53, '/'),
    (57, ' '),
];

pub fn representative_key(keycode: u32) -> Option<char> {
    QWERTY_CODES
        .iter()
        .find_map(|(code, key)| (*code == keycode).then_some(*key))
}

pub fn representative_keycode(key: char) -> Option<u32> {
    QWERTY_CODES
        .iter()
        .find_map(|(code, candidate)| (*candidate == key).then_some(*code))
}

pub fn colemak_target(key: char) -> Option<char> {
    let target = match key {
        'e' => 'f',
        'r' => 'p',
        't' => 'g',
        'y' => 'j',
        'u' => 'l',
        'i' => 'u',
        'o' => 'y',
        'p' => ';',
        's' => 'r',
        'd' => 's',
        'f' => 't',
        'g' => 'd',
        'j' => 'n',
        'k' => 'e',
        'l' => 'i',
        ';' => 'o',
        'n' => 'k',
        other if representative_keycode(other).is_some() => other,
        _ => return None,
    };
    Some(target)
}

pub fn colemak_output(key: char, shifted: bool) -> Option<char> {
    us_printable(colemak_target(key)?, shifted)
}

pub fn colemak_forward(key: char, shifted: bool) -> Option<(u32, u32)> {
    let target = colemak_target(key)?;
    Some((
        us_printable(target, shifted)? as u32,
        representative_keycode(target)?,
    ))
}

pub fn hangul_input(key: char, shifted: bool) -> Option<Input> {
    let jamo = match (key, shifted) {
        ('1', false) => Some(Jamo::Final('ㅎ')),
        ('2', false) => Some(Jamo::Final('ㅆ')),
        ('3', false) => Some(Jamo::Final('ㅂ')),
        ('4', false) => Some(Jamo::Medial('ㅛ')),
        ('5', false) => Some(Jamo::Medial('ㅠ')),
        ('6', false) => Some(Jamo::Medial('ㅑ')),
        ('7', false) => Some(Jamo::Medial('ㅖ')),
        ('8', false) => Some(Jamo::Medial('ㅢ')),
        ('9', false) => Some(Jamo::Medial('ㅜ')),
        ('0', false) => Some(Jamo::Initial('ㅋ')),
        ('q', false) => Some(Jamo::Final('ㅅ')),
        ('w', false) => Some(Jamo::Final('ㄹ')),
        ('e', false) => Some(Jamo::Medial('ㅕ')),
        ('r', false) => Some(Jamo::Medial('ㅐ')),
        ('t', false) => Some(Jamo::Medial('ㅓ')),
        ('y', false) => Some(Jamo::Initial('ㄹ')),
        ('u', false) => Some(Jamo::Initial('ㄷ')),
        ('i', false) => Some(Jamo::Initial('ㅁ')),
        ('o', false) => Some(Jamo::Initial('ㅊ')),
        ('p', false) => Some(Jamo::Initial('ㅍ')),
        ('a', false) => Some(Jamo::Final('ㅇ')),
        ('s', false) => Some(Jamo::Final('ㄴ')),
        ('d', false) => Some(Jamo::Medial('ㅣ')),
        ('f', false) => Some(Jamo::Medial('ㅏ')),
        ('g', false) => Some(Jamo::Medial('ㅡ')),
        ('h', false) => Some(Jamo::Initial('ㄴ')),
        ('j', false) => Some(Jamo::Initial('ㅇ')),
        ('k', false) => Some(Jamo::Initial('ㄱ')),
        ('l', false) => Some(Jamo::Initial('ㅈ')),
        (';', false) => Some(Jamo::Initial('ㅂ')),
        ('\'', false) => Some(Jamo::Initial('ㅌ')),
        ('z', false) => Some(Jamo::Final('ㅁ')),
        ('x', false) => Some(Jamo::Final('ㄱ')),
        ('c', false) => Some(Jamo::Medial('ㅔ')),
        ('v', false) => Some(Jamo::Medial('ㅗ')),
        ('b', false) => Some(Jamo::Medial('ㅜ')),
        ('n', false) => Some(Jamo::Initial('ㅅ')),
        ('m', false) => Some(Jamo::Initial('ㅎ')),
        ('/', false) => Some(Jamo::Medial('ㅗ')),
        ('1', true) => Some(Jamo::Final('ㄲ')),
        ('2', true) => Some(Jamo::Final('ㄺ')),
        ('3', true) => Some(Jamo::Final('ㅈ')),
        ('4', true) => Some(Jamo::Final('ㄿ')),
        ('5', true) => Some(Jamo::Final('ㄾ')),
        ('q', true) => Some(Jamo::Final('ㅍ')),
        ('w', true) => Some(Jamo::Final('ㅌ')),
        ('e', true) => Some(Jamo::Final('ㄵ')),
        ('r', true) => Some(Jamo::Final('ㅀ')),
        ('t', true) => Some(Jamo::Final('ㄽ')),
        ('a', true) => Some(Jamo::Final('ㄷ')),
        ('s', true) => Some(Jamo::Final('ㄶ')),
        ('d', true) => Some(Jamo::Final('ㄼ')),
        ('f', true) => Some(Jamo::Final('ㄻ')),
        ('g', true) => Some(Jamo::Medial('ㅒ')),
        ('z', true) => Some(Jamo::Final('ㅊ')),
        ('x', true) => Some(Jamo::Final('ㅄ')),
        ('c', true) => Some(Jamo::Final('ㅋ')),
        ('v', true) => Some(Jamo::Final('ㄳ')),
        _ => None,
    };
    if let Some(jamo) = jamo {
        return Some(Input::Jamo(JamoInput {
            jamo,
            source_key: key,
        }));
    }

    let output = match (key, shifted) {
        ('y', true) => '5',
        ('u', true) => '6',
        ('i', true) => '7',
        ('o', true) => '8',
        ('p', true) => '9',
        ('h', true) => '0',
        ('j', true) => '1',
        ('k', true) => '2',
        ('l', true) => '3',
        (';', true) => '4',
        ('b', true) => '?',
        ('n', true) => ':',
        ('m', true) => ';',
        ('/', true) => '!',
        _ => us_printable(key, shifted)?,
    };
    Some(Input::Emit(output))
}

pub fn us_printable(key: char, shifted: bool) -> Option<char> {
    if !shifted {
        return representative_keycode(key).is_some().then_some(key);
    }
    Some(match key {
        'a'..='z' => key.to_ascii_uppercase(),
        '`' => '~',
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        ' ' => ' ',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evdev_codes_identify_qwerty_positions_bidirectionally() {
        for (keycode, key) in QWERTY_CODES {
            assert_eq!(representative_key(*keycode), Some(*key));
            assert_eq!(representative_keycode(*key), Some(*keycode));
        }
        assert_eq!(representative_key(0), None);
    }

    #[test]
    fn colemak_mapping_uses_shift_not_caps_lock() {
        assert_eq!(colemak_output('e', false), Some('f'));
        assert_eq!(colemak_output('e', true), Some('F'));
        assert_eq!(colemak_output('p', false), Some(';'));
        assert_eq!(colemak_output('p', true), Some(':'));
        assert_eq!(colemak_output('1', true), Some('!'));
        assert_eq!(colemak_forward('e', false), Some((b'f' as u32, 33)));
        assert_eq!(colemak_forward('n', false), Some((b'k' as u32, 37)));
    }

    #[test]
    fn sane_punctuation_is_not_colemak() {
        assert_eq!(hangul_input('n', true), Some(Input::Emit(':')));
        assert_eq!(hangul_input('m', true), Some(Input::Emit(';')));
        assert_eq!(hangul_input('/', true), Some(Input::Emit('!')));
        assert_eq!(hangul_input('b', true), Some(Input::Emit('?')));
        assert_eq!(hangul_input(';', true), Some(Input::Emit('4')));
    }

    #[test]
    fn all_representative_keys_have_both_layout_levels() {
        for (_, key) in QWERTY_CODES {
            assert!(colemak_output(*key, false).is_some(), "Colemak base {key}");
            assert!(colemak_output(*key, true).is_some(), "Colemak shift {key}");
            assert!(hangul_input(*key, false).is_some(), "Hangul base {key}");
            assert!(hangul_input(*key, true).is_some(), "Hangul shift {key}");
        }
    }
}
