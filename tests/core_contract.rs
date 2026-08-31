use lisle::composition::{Input, Jamo};
use lisle::engine::{
    Action, CONTROL_MASK, InputState, KeyEvent, LisleEngine, RELEASE_MASK, SHIFT_MASK, keysym,
};
use lisle::key::{colemak_output, hangul_input, representative_keycode, us_printable};

fn assert_layout_row(keys: &str, expected: &str, shifted: bool, hangul: bool) {
    let keys = keys.chars().collect::<Vec<_>>();
    let expected = expected.chars().collect::<Vec<_>>();
    assert_eq!(keys.len(), expected.len());
    for (key, expected) in keys.into_iter().zip(expected) {
        let actual = if hangul {
            match hangul_input(key, shifted).expect("complete Hangul map") {
                Input::Emit(value) => value,
                Input::Jamo(input) => match input.jamo {
                    Jamo::Initial(value) | Jamo::Medial(value) | Jamo::Final(value) => value,
                },
            }
        } else {
            colemak_output(key, shifted).expect("complete Colemak map")
        };
        assert_eq!(actual, expected, "key={key:?}, shifted={shifted}");
    }
}

fn assert_slot_row(keys: &str, expected: &str, shifted: bool) {
    let keys = keys.chars().collect::<Vec<_>>();
    let expected = expected.chars().collect::<Vec<_>>();
    assert_eq!(keys.len(), expected.len());
    for (key, expected) in keys.into_iter().zip(expected) {
        let actual = match hangul_input(key, shifted).expect("complete Hangul map") {
            Input::Emit(_) => '-',
            Input::Jamo(input) => match input.jamo {
                Jamo::Initial(_) => 'I',
                Jamo::Medial(_) => 'M',
                Jamo::Final(_) => 'F',
            },
        };
        assert_eq!(actual, expected, "key={key:?}, shifted={shifted}");
    }
}

#[test]
fn complete_layout_tables_match_the_normative_specification() {
    for (keys, base, shifted) in [
        ("`1234567890-=", "`1234567890-=", "~!@#$%^&*()_+"),
        ("qwertyuiop[]\\", "qwfpgjluy;[]\\", "QWFPGJLUY:{}|"),
        ("asdfghjkl;'", "arstdhneio'", "ARSTDHNEIO\""),
        ("zxcvbnm,./", "zxcvbkm,./", "ZXCVBKM<>?"),
        (" ", " ", " "),
    ] {
        assert_layout_row(keys, base, false, false);
        assert_layout_row(keys, shifted, true, false);
    }

    for (keys, base, shifted) in [
        (
            "`1234567890-=",
            "`ㅎㅆㅂㅛㅠㅑㅖㅢㅜㅋ-=",
            "~ㄲㄺㅈㄿㄾ^&*()_+",
        ),
        (
            "qwertyuiop[]\\",
            "ㅅㄹㅕㅐㅓㄹㄷㅁㅊㅍ[]\\",
            "ㅍㅌㄵㅀㄽ56789{}|",
        ),
        ("asdfghjkl;'", "ㅇㄴㅣㅏㅡㄴㅇㄱㅈㅂㅌ", "ㄷㄶㄼㄻㅒ01234\""),
        ("zxcvbnm,./", "ㅁㄱㅔㅗㅜㅅㅎ,.ㅗ", "ㅊㅄㅋㄳ?:;<>!"),
        (" ", " ", " "),
    ] {
        assert_layout_row(keys, base, false, true);
        assert_layout_row(keys, shifted, true, true);
    }

    for (keys, base, shifted) in [
        ("`1234567890-=", "-FFFMMMMMMI--", "-FFFFF-------"),
        ("qwertyuiop[]\\", "FFMMMIIIII---", "FFFFF--------"),
        ("asdfghjkl;'", "FFMMMIIIIII", "FFFFM------"),
        ("zxcvbnm,./", "FFMMMII--M", "FFFF------"),
        (" ", "-", "-"),
    ] {
        assert_slot_row(keys, base, false);
        assert_slot_row(keys, shifted, true);
    }
}

#[derive(Default)]
struct Harness {
    engine: LisleEngine,
    committed: String,
    preedit: String,
    forwarded: Vec<KeyEvent>,
}

impl Harness {
    fn send(&mut self, event: KeyEvent) -> bool {
        let (handled, actions) = self.engine.process(event);
        for action in actions {
            match action {
                Action::Commit(text) => self.committed.push_str(&text),
                Action::Preedit(text) => self.preedit = text,
                Action::Forward {
                    keyval,
                    keycode,
                    state,
                } => self.forwarded.push(KeyEvent::new(keyval, keycode, state)),
            }
        }
        if !handled {
            self.forwarded.push(event);
        }
        handled
    }

    fn select_hangul(&mut self) {
        self.send(KeyEvent::new(keysym::SHIFT_R, 54, SHIFT_MASK));
        self.send(KeyEvent::new(
            keysym::SHIFT_R,
            54,
            SHIFT_MASK | RELEASE_MASK,
        ));
        assert_eq!(self.engine.state(), InputState::Hangul);
    }

    fn key(&mut self, key: char) {
        let keycode = representative_keycode(key).expect("representative key");
        let keyval = us_printable(key, false).expect("US key") as u32;
        self.send(KeyEvent::new(keyval, keycode, 0));
        self.send(KeyEvent::new(keyval, keycode, RELEASE_MASK));
    }
}

fn hangul_text(sequence: &str) -> String {
    let mut harness = Harness::default();
    harness.select_hangul();
    for key in sequence.chars().filter(|value| !value.is_whitespace()) {
        harness.key(key);
    }
    harness.key(' ');
    assert!(harness.preedit.is_empty());
    harness
        .committed
        .strip_suffix(' ')
        .expect("space boundary")
        .to_owned()
}

#[test]
fn all_normative_composition_examples_run_through_the_full_engine() {
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
        ("ifamr2jtbb", "망했어ㅜㅜ"),
    ] {
        assert_eq!(hangul_text(sequence), expected, "{sequence}");
    }
}

#[test]
fn commit_preedit_and_boundary_order_match_the_specification() {
    let mut harness = Harness::default();
    harness.select_hangul();
    harness.key('k');
    assert_eq!(harness.committed, "");
    assert_eq!(harness.preedit, "ㄱ");
    harness.key('f');
    assert_eq!(harness.committed, "");
    assert_eq!(harness.preedit, "가");
    harness.key('x');
    harness.key('f');
    assert_eq!(harness.committed, "각");
    assert_eq!(harness.preedit, "ㅏ");
    harness.key(' ');
    assert_eq!(harness.committed, "각ㅏ ");
    assert_eq!(harness.preedit, "");
}

#[test]
fn space_commits_flushed_preedit_and_space_in_one_action() {
    let mut engine = LisleEngine::default();
    engine.process(KeyEvent::new(keysym::SHIFT_R, 54, SHIFT_MASK));
    engine.process(KeyEvent::new(
        keysym::SHIFT_R,
        54,
        SHIFT_MASK | RELEASE_MASK,
    ));

    for key in ['k', 'f'] {
        let keycode = representative_keycode(key).expect("representative key");
        engine.process(KeyEvent::new(key as u32, keycode, 0));
    }
    let space = representative_keycode(' ').expect("space keycode");
    assert_eq!(
        engine.process(KeyEvent::new(' ' as u32, space, 0)),
        (
            true,
            vec![Action::Preedit(String::new()), Action::Commit("가 ".into()),]
        )
    );
}

#[test]
fn unknown_physical_identity_flushes_without_guessing_from_keyval() {
    let mut harness = Harness::default();
    harness.select_hangul();
    harness.key('k');
    harness.key('f');
    let unknown = KeyEvent::new(b'k' as u32, 0, 0);
    assert!(!harness.send(unknown));
    assert_eq!(harness.committed, "가");
    assert_eq!(harness.preedit, "");
    assert_eq!(harness.forwarded.last(), Some(&unknown));
}

#[test]
fn key_delivery_is_never_both_passed_through_and_synthetically_forwarded() {
    let representative = "`1234567890-=qwertyuiop[]\\asdfghjkl;'zxcvbnm,./ ";
    for mode in [InputState::Roman, InputState::Hangul] {
        for key in representative.chars() {
            let mut engine = LisleEngine::default();
            if mode == InputState::Hangul {
                let mut harness = Harness {
                    engine,
                    ..Harness::default()
                };
                harness.select_hangul();
                engine = harness.engine;
            }
            let event = KeyEvent::new(
                us_printable(key, false).expect("US key") as u32,
                representative_keycode(key).expect("keycode"),
                0,
            );
            let (handled, actions) = engine.process(event);
            let forwarded = actions
                .iter()
                .filter(|action| matches!(action, Action::Forward { .. }))
                .count();
            assert!(forwarded <= 1);
            assert!(handled || forwarded == 0);
        }
    }
}

#[test]
fn shortcuts_preserve_the_xkb_mapped_event_by_passing_it_through() {
    let mut engine = LisleEngine::default();
    let state = CONTROL_MASK | SHIFT_MASK | lisle::engine::LOCK_MASK;
    assert_eq!(
        engine.process(KeyEvent::new(b'F' as u32, 18, state)),
        (false, Vec::new())
    );
}

#[test]
fn reported_alt_g_and_control_k_shortcuts_use_colemak_meanings() {
    for (physical, modifier, xkb_mapped) in [
        ('g', lisle::engine::MOD1_MASK, 'd'),
        ('k', CONTROL_MASK, 'e'),
    ] {
        let mut engine = LisleEngine::default();
        let keycode = representative_keycode(physical).expect("physical keycode");
        assert_eq!(
            engine.process(KeyEvent::new(xkb_mapped as u32, keycode, modifier)),
            (false, Vec::new())
        );
        assert_eq!(
            engine.process(KeyEvent::new(
                xkb_mapped as u32,
                keycode,
                modifier | RELEASE_MASK,
            )),
            (false, Vec::new())
        );
    }
}

#[test]
fn emitted_text_never_contains_conjoining_jamo() {
    for sequence in ["kxf", "xkf", "jkf", "/f", "9t", "kfxf", "kfqw"] {
        assert!(
            hangul_text(sequence)
                .chars()
                .all(|value| !(0x1100..=0x11ff).contains(&(value as u32))),
            "{sequence}"
        );
    }
}

#[test]
fn ending_a_context_discards_state_and_starts_the_next_context_in_roman() {
    let mut engine = LisleEngine::default();
    engine.process(KeyEvent::new(keysym::SHIFT_R, 54, SHIFT_MASK));
    engine.process(KeyEvent::new(
        keysym::SHIFT_R,
        54,
        SHIFT_MASK | RELEASE_MASK,
    ));
    engine.process(KeyEvent::new(b'k' as u32, 37, 0));
    engine.end_context();
    assert_eq!(engine.state(), InputState::Roman);
    assert_eq!(
        engine.process(KeyEvent::new(b'f' as u32, 18, 0)),
        (false, Vec::new())
    );
}

#[test]
fn roman_events_are_xkb_mapped_and_pass_through_without_text_actions() {
    let mut engine = LisleEngine::default();
    for input in [
        KeyEvent::new(b'f' as u32, 18, 0),
        KeyEvent::new(b'f' as u32, 18, 0),
        KeyEvent::new(b'f' as u32, 18, RELEASE_MASK),
        KeyEvent::new(b';' as u32, 25, 0),
        KeyEvent::new(b';' as u32, 25, RELEASE_MASK),
        KeyEvent::new(b' ' as u32, 57, 0),
        KeyEvent::new(b' ' as u32, 57, RELEASE_MASK),
        KeyEvent::new(keysym::BACK_SPACE, 58, lisle::engine::LOCK_MASK),
        KeyEvent::new(
            keysym::BACK_SPACE,
            58,
            lisle::engine::LOCK_MASK | RELEASE_MASK,
        ),
        KeyEvent::new(b'f' as u32, 18, lisle::engine::MOD2_MASK),
        KeyEvent::new(b'f' as u32, 18, lisle::engine::MOD2_MASK | RELEASE_MASK),
    ] {
        assert_eq!(engine.process(input), (false, Vec::new()), "{input:?}");
    }
}
