use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;

use xkbcommon::xkb::{self, compose};

use crate::composition::{Composer, Input};
use crate::key::{colemak_output, hangul_input, representative_key};

pub const SHIFT_MASK: u32 = 1;
pub const LOCK_MASK: u32 = 1 << 1;
pub const CONTROL_MASK: u32 = 1 << 2;
pub const MOD1_MASK: u32 = 1 << 3;
pub const MOD2_MASK: u32 = 1 << 4;
pub const MOD4_MASK: u32 = 1 << 6;
pub const HANDLED_MASK: u32 = 1 << 24;
pub const FORWARD_MASK: u32 = 1 << 25;
pub const SUPER_MASK: u32 = 1 << 26;
pub const HYPER_MASK: u32 = 1 << 27;
pub const META_MASK: u32 = 1 << 28;
pub const RELEASE_MASK: u32 = 1 << 30;

const NON_TEXT_IGNORED_MASK: u32 =
    SHIFT_MASK | LOCK_MASK | MOD2_MASK | RELEASE_MASK | HANDLED_MASK | FORWARD_MASK;

pub mod keysym {
    pub const BACK_SPACE: u32 = 0xff08;
    pub const TAB: u32 = 0xff09;
    pub const RETURN: u32 = 0xff0d;
    pub const ESCAPE: u32 = 0xff1b;
    pub const MULTI_KEY: u32 = 0xff20;
    pub const HOME: u32 = 0xff50;
    pub const LEFT: u32 = 0xff51;
    pub const UP: u32 = 0xff52;
    pub const RIGHT: u32 = 0xff53;
    pub const DOWN: u32 = 0xff54;
    pub const PAGE_UP: u32 = 0xff55;
    pub const PAGE_DOWN: u32 = 0xff56;
    pub const END: u32 = 0xff57;
    pub const INSERT: u32 = 0xff63;
    pub const F1: u32 = 0xffbe;
    pub const F12: u32 = 0xffc9;
    pub const SHIFT_L: u32 = 0xffe1;
    pub const SHIFT_R: u32 = 0xffe2;
    pub const CAPS_LOCK: u32 = 0xffe5;
    pub const DELETE: u32 = 0xffff;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyEvent {
    pub keyval: u32,
    pub keycode: u32,
    pub state: u32,
}

impl KeyEvent {
    pub const fn new(keyval: u32, keycode: u32, state: u32) -> Self {
        Self {
            keyval,
            keycode,
            state,
        }
    }

    fn is_release(self) -> bool {
        self.state & RELEASE_MASK != 0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InputState {
    Hangul,
    #[default]
    Roman,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShiftSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HeldShift {
    side: ShiftSide,
    press: KeyEvent,
    forwarded: bool,
}

#[derive(Clone, Debug, Default)]
struct ShiftTracker {
    held: Vec<HeldShift>,
    candidate: Option<(ShiftSide, u32)>,
    cancelled: bool,
}

impl ShiftTracker {
    fn clear(&mut self) {
        self.held.clear();
        self.candidate = None;
        self.cancelled = false;
    }

    fn cancel(&mut self) {
        if self.candidate.take().is_some() {
            self.cancelled = true;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReleaseRoute {
    Consume,
    PassThrough,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HostCompose {
    keysyms: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Commit(String),
    Preedit(String),
    Forward {
        keyval: u32,
        keycode: u32,
        state: u32,
    },
}

#[derive(Default)]
pub struct LisleEngine {
    state: InputState,
    composer: Composer,
    shifts: ShiftTracker,
    release_routes: HashMap<u32, Vec<ReleaseRoute>>,
    host_compose: Option<HostCompose>,
}

impl LisleEngine {
    pub fn state(&self) -> InputState {
        self.state
    }

    pub fn process(&mut self, event: KeyEvent) -> (bool, Vec<Action>) {
        if event.is_release()
            && event.keycode != 0
            && self.release_routes.contains_key(&event.keycode)
        {
            return self.process_release(event);
        }

        if self.host_compose.is_some() {
            if let Some(side) = shift_side(event.keyval)
                && self
                    .shifts
                    .held
                    .iter()
                    .any(|held| held.press.keycode == event.keycode)
            {
                return self.process_shift(event, side);
            }
            return self.process_host_compose(event);
        }

        if let Some(side) = shift_side(event.keyval) {
            return self.process_shift(event, side);
        }

        let cancels_shift = event.keyval != keysym::CAPS_LOCK && !self.shifts.held.is_empty();
        if cancels_shift {
            self.shifts.cancel();
        }

        let (handled, mut actions) = if event.is_release() {
            self.process_release(event)
        } else {
            let (handled, actions, route) = self.process_press(event);
            self.remember_release_route(event.keycode, route);
            (handled, actions)
        };

        if cancels_shift
            && (!handled
                || actions
                    .iter()
                    .any(|action| matches!(action, Action::Forward { .. })))
        {
            let mut replay = self.replay_held_shifts();
            replay.append(&mut actions);
            actions = replay;
        }
        (handled, actions)
    }

    pub fn focus_in(&mut self) {
        self.state = InputState::Roman;
        self.clear_transient();
    }

    pub fn reset(&mut self) {
        self.composer.clear();
        self.host_compose = None;
        if self.shifts.held.is_empty() {
            self.shifts.clear();
        } else {
            self.shifts.cancel();
            self.shifts.cancelled = true;
        }
    }

    pub fn end_context(&mut self) {
        self.state = InputState::Roman;
        self.clear_transient();
    }

    fn clear_transient(&mut self) {
        self.composer.clear();
        self.shifts.clear();
        self.release_routes.clear();
        self.host_compose = None;
    }

    fn process_press(&mut self, event: KeyEvent) -> (bool, Vec<Action>, ReleaseRoute) {
        if event.keyval == keysym::ESCAPE {
            let actions = self.flush_actions();
            self.state = InputState::Roman;
            return (false, actions, ReleaseRoute::PassThrough);
        }

        if is_dead_or_compose(event.keyval) {
            let sequence = HostCompose {
                keysyms: vec![event.keyval],
            };
            if compose_is_active(&sequence.keysyms).unwrap_or(true) {
                self.host_compose = Some(sequence);
            }
            return (false, self.flush_actions(), ReleaseRoute::PassThrough);
        }

        let non_text_modifiers = event.state & !NON_TEXT_IGNORED_MASK;
        if non_text_modifiers != 0 {
            return (false, self.flush_actions(), ReleaseRoute::PassThrough);
        }

        if event.keyval == keysym::BACK_SPACE {
            if self.composer.backspace() {
                return (
                    true,
                    vec![Action::Preedit(self.composer.preedit())],
                    ReleaseRoute::Consume,
                );
            }
            return (false, Vec::new(), ReleaseRoute::PassThrough);
        }

        if is_boundary_key(event.keyval) {
            return (false, self.flush_actions(), ReleaseRoute::PassThrough);
        }

        let Some(key) = representative_key(event.keycode) else {
            return (false, self.flush_actions(), ReleaseRoute::PassThrough);
        };
        let shifted = event.state & SHIFT_MASK != 0;

        match self.state {
            InputState::Roman => match colemak_output(key, shifted) {
                Some(output) => (
                    true,
                    vec![Action::Commit(output.to_string())],
                    ReleaseRoute::Consume,
                ),
                None => (false, Vec::new(), ReleaseRoute::PassThrough),
            },
            InputState::Hangul => match hangul_input(key, shifted) {
                Some(Input::Jamo(jamo)) => {
                    let transition = self.composer.push(jamo);
                    let mut actions = Vec::new();
                    if !transition.committed.is_empty() {
                        actions.push(Action::Commit(transition.committed));
                    }
                    actions.push(Action::Preedit(transition.preedit));
                    (true, actions, ReleaseRoute::Consume)
                }
                Some(Input::Emit(output)) => {
                    let mut actions = self.flush_actions();
                    actions.push(Action::Commit(output.to_string()));
                    (true, actions, ReleaseRoute::Consume)
                }
                None => (false, Vec::new(), ReleaseRoute::PassThrough),
            },
        }
    }

    fn process_release(&mut self, event: KeyEvent) -> (bool, Vec<Action>) {
        let Some(routes) = self.release_routes.remove(&event.keycode) else {
            return (false, Vec::new());
        };
        if routes.is_empty() {
            return (true, Vec::new());
        }
        if routes.contains(&ReleaseRoute::PassThrough) {
            return (false, Vec::new());
        }
        (true, Vec::new())
    }

    fn process_host_compose(&mut self, event: KeyEvent) -> (bool, Vec<Action>) {
        if event.is_release() {
            return self.process_release(event);
        }

        if event.keyval == keysym::ESCAPE {
            self.host_compose = None;
            self.state = InputState::Roman;
        } else if !is_modifier_key(event.keyval)
            && let Some(mut sequence) = self.host_compose.take()
        {
            sequence.keysyms.push(event.keyval);
            if compose_is_active(&sequence.keysyms).unwrap_or(false) {
                self.host_compose = Some(sequence);
            }
        }
        self.remember_release_route(event.keycode, ReleaseRoute::PassThrough);
        (false, Vec::new())
    }

    fn remember_release_route(&mut self, keycode: u32, route: ReleaseRoute) {
        if keycode == 0 {
            return;
        }

        let routes = self.release_routes.entry(keycode).or_default();
        if route != ReleaseRoute::Consume && !routes.contains(&route) {
            routes.push(route);
        }
    }

    fn process_shift(&mut self, event: KeyEvent, side: ShiftSide) -> (bool, Vec<Action>) {
        if event.is_release() {
            let Some(index) = self
                .shifts
                .held
                .iter()
                .position(|held| held.press.keycode == event.keycode)
            else {
                self.shifts.cancel();
                self.shifts.cancelled = true;
                return (false, Vec::new());
            };
            let held = self.shifts.held.remove(index);
            let valid_tap = !self.shifts.cancelled
                && self.shifts.candidate == Some((side, event.keycode))
                && held.side == side
                && self.shifts.held.is_empty()
                && meaningful_modifiers(event.state) == 0;

            if self.shifts.held.is_empty() {
                self.shifts.candidate = None;
                self.shifts.cancelled = false;
            }
            if !valid_tap {
                return (!held.forwarded, Vec::new());
            }

            let selected = match side {
                ShiftSide::Left => InputState::Roman,
                ShiftSide::Right => InputState::Hangul,
            };
            if self.state == selected {
                return (true, Vec::new());
            }
            let actions = self.flush_actions();
            self.state = selected;
            return (true, actions);
        }

        if let Some(held) = self
            .shifts
            .held
            .iter()
            .find(|held| held.press.keycode == event.keycode)
            .copied()
        {
            self.shifts.cancel();
            self.shifts.cancelled = true;
            return (!held.forwarded, Vec::new());
        }

        let meaningful = meaningful_modifiers(event.state);
        if self.shifts.held.is_empty() && meaningful == 0 {
            self.shifts.held.push(HeldShift {
                side,
                press: event,
                forwarded: false,
            });
            self.shifts.candidate = Some((side, event.keycode));
            return (true, Vec::new());
        }

        if self.shifts.held.is_empty() {
            self.shifts.held.push(HeldShift {
                side,
                press: event,
                forwarded: true,
            });
            self.shifts.cancelled = true;
            return (false, Vec::new());
        }

        let forward_now = self.shifts.held.iter().any(|held| held.forwarded);
        self.shifts.cancel();
        self.shifts.cancelled = true;
        self.shifts.held.push(HeldShift {
            side,
            press: event,
            forwarded: forward_now,
        });
        (!forward_now, Vec::new())
    }

    fn replay_held_shifts(&mut self) -> Vec<Action> {
        self.shifts
            .held
            .iter_mut()
            .filter_map(|held| {
                if held.forwarded {
                    return None;
                }
                held.forwarded = true;
                Some(forward(held.press))
            })
            .collect()
    }

    fn flush_actions(&mut self) -> Vec<Action> {
        let text = self.composer.flush();
        if text.is_empty() {
            Vec::new()
        } else {
            vec![Action::Commit(text), Action::Preedit(String::new())]
        }
    }
}

fn meaningful_modifiers(state: u32) -> u32 {
    state & !NON_TEXT_IGNORED_MASK
}

fn forward(event: KeyEvent) -> Action {
    Action::Forward {
        keyval: event.keyval,
        keycode: event.keycode,
        state: event.state,
    }
}

fn shift_side(keyval: u32) -> Option<ShiftSide> {
    match keyval {
        keysym::SHIFT_L => Some(ShiftSide::Left),
        keysym::SHIFT_R => Some(ShiftSide::Right),
        _ => None,
    }
}

fn is_dead_or_compose(keyval: u32) -> bool {
    keyval == keysym::MULTI_KEY || (0xfe50..=0xfeff).contains(&keyval)
}

thread_local! {
    static COMPOSE_STATE: RefCell<Option<compose::State>> = RefCell::new(new_compose_state());
}

fn new_compose_state() -> Option<compose::State> {
    let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .find_map(|name| env::var_os(name).filter(|value| !value.is_empty()));

    locale
        .as_deref()
        .and_then(|locale| compose_table(&context, locale))
        .or_else(|| compose_table(&context, OsStr::new("C")))
        .map(|table| compose::State::new(&table, compose::STATE_NO_FLAGS))
}

fn compose_table(context: &xkb::Context, locale: &OsStr) -> Option<compose::Table> {
    compose::Table::new_from_locale(context, locale, compose::COMPILE_NO_FLAGS).ok()
}

fn compose_is_active(keysyms: &[u32]) -> Option<bool> {
    COMPOSE_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut()?;
        state.reset();
        for &keysym in keysyms {
            state.feed(xkb::Keysym::new(keysym));
        }
        let active = state.status() == compose::Status::Composing;
        state.reset();
        Some(active)
    })
}

fn is_modifier_key(keyval: u32) -> bool {
    (0xffe1..=0xffee).contains(&keyval) || (0xfe01..=0xfe13).contains(&keyval)
}

fn is_boundary_key(keyval: u32) -> bool {
    matches!(
        keyval,
        keysym::DELETE
            | keysym::LEFT
            | keysym::RIGHT
            | keysym::UP
            | keysym::DOWN
            | keysym::HOME
            | keysym::END
            | keysym::PAGE_UP
            | keysym::PAGE_DOWN
            | keysym::INSERT
            | keysym::TAB
            | keysym::RETURN
    ) || (keysym::F1..=keysym::F12).contains(&keyval)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn event(keyval: u32, keycode: u32, state: u32) -> KeyEvent {
        KeyEvent::new(keyval, keycode, state)
    }

    fn select_hangul(engine: &mut LisleEngine) {
        assert_eq!(
            engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK)),
            (true, Vec::new())
        );
        assert_eq!(
            engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK | RELEASE_MASK)),
            (true, Vec::new())
        );
        assert_eq!(engine.state(), InputState::Hangul);
    }

    #[test]
    fn starts_in_roman_and_shift_taps_select_absolute_modes() {
        let mut engine = LisleEngine::default();
        assert_eq!(engine.state(), InputState::Roman);
        select_hangul(&mut engine);
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK));
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK | RELEASE_MASK));
        assert_eq!(engine.state(), InputState::Hangul);
        engine.process(event(keysym::SHIFT_L, 42, SHIFT_MASK));
        engine.process(event(keysym::SHIFT_L, 42, SHIFT_MASK | RELEASE_MASK));
        assert_eq!(engine.state(), InputState::Roman);
    }

    #[test]
    fn shifted_printable_cancels_tap_without_forwarding_bare_shift() {
        let mut engine = LisleEngine::default();
        select_hangul(&mut engine);
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK));
        assert_eq!(
            engine.process(event(b'K' as u32, 37, SHIFT_MASK)),
            (true, vec![Action::Commit("2".into())])
        );
        assert_eq!(
            engine.process(event(b'K' as u32, 37, SHIFT_MASK | RELEASE_MASK)),
            (true, Vec::new())
        );
        assert_eq!(
            engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK | RELEASE_MASK)),
            (true, Vec::new())
        );
        assert_eq!(engine.state(), InputState::Hangul);
    }

    #[test]
    fn repeated_shift_press_cannot_rearm_tap() {
        let mut engine = LisleEngine::default();
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK));
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK));
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK));
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK | RELEASE_MASK));
        assert_eq!(engine.state(), InputState::Roman);
    }

    #[test]
    fn both_shifts_cancel_selection() {
        let mut engine = LisleEngine::default();
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK));
        engine.process(event(keysym::SHIFT_L, 42, SHIFT_MASK));
        engine.process(event(keysym::SHIFT_L, 42, SHIFT_MASK | RELEASE_MASK));
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK | RELEASE_MASK));
        assert_eq!(engine.state(), InputState::Roman);
    }

    #[test]
    fn second_shift_passes_when_host_already_saw_first_shift() {
        let mut engine = LisleEngine::default();
        for input in [
            event(keysym::SHIFT_R, 54, CONTROL_MASK | SHIFT_MASK),
            event(keysym::SHIFT_L, 42, CONTROL_MASK | SHIFT_MASK),
            event(
                keysym::SHIFT_L,
                42,
                CONTROL_MASK | SHIFT_MASK | RELEASE_MASK,
            ),
            event(
                keysym::SHIFT_R,
                54,
                CONTROL_MASK | SHIFT_MASK | RELEASE_MASK,
            ),
        ] {
            assert_eq!(engine.process(input), (false, Vec::new()));
        }
    }

    #[test]
    fn caps_lock_does_not_cancel_shift_tap() {
        let mut engine = LisleEngine::default();
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK));
        assert_eq!(
            engine.process(event(keysym::CAPS_LOCK, 58, SHIFT_MASK | LOCK_MASK)),
            (false, Vec::new())
        );
        assert_eq!(
            engine.process(event(
                keysym::CAPS_LOCK,
                58,
                SHIFT_MASK | LOCK_MASK | RELEASE_MASK,
            )),
            (false, Vec::new())
        );
        engine.process(event(
            keysym::SHIFT_R,
            54,
            SHIFT_MASK | LOCK_MASK | RELEASE_MASK,
        ));
        assert_eq!(engine.state(), InputState::Hangul);
    }

    #[test]
    fn wrong_shift_release_and_modifier_change_cancel_tap() {
        let mut engine = LisleEngine::default();
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK));
        assert_eq!(
            engine.process(event(keysym::SHIFT_L, 42, SHIFT_MASK | RELEASE_MASK)),
            (false, Vec::new())
        );
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK | RELEASE_MASK));
        assert_eq!(engine.state(), InputState::Roman);

        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK));
        let (handled, actions) = engine.process(event(0xffe3, 29, SHIFT_MASK | CONTROL_MASK));
        assert!(!handled);
        assert_eq!(
            actions,
            vec![forward(event(keysym::SHIFT_R, 54, SHIFT_MASK))]
        );
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK | RELEASE_MASK));
        assert_eq!(engine.state(), InputState::Roman);
    }

    #[test]
    fn selecting_active_hangul_mode_preserves_preedit() {
        let mut engine = LisleEngine::default();
        select_hangul(&mut engine);
        assert_eq!(
            engine.process(event(b'k' as u32, 37, 0)),
            (true, vec![Action::Preedit("ㄱ".into())])
        );
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK));
        assert_eq!(
            engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK | RELEASE_MASK)),
            (true, Vec::new())
        );
        assert_eq!(
            engine.process(event(b'f' as u32, 33, 0)),
            (true, vec![Action::Preedit("가".into())])
        );
    }

    #[test]
    fn cancelled_shift_is_replayed_before_host_action() {
        let mut engine = LisleEngine::default();
        engine.process(event(keysym::SHIFT_L, 42, SHIFT_MASK));
        assert_eq!(
            engine.process(event(keysym::TAB, 15, SHIFT_MASK)),
            (
                false,
                vec![Action::Forward {
                    keyval: keysym::SHIFT_L,
                    keycode: 42,
                    state: SHIFT_MASK,
                }]
            )
        );
        assert_eq!(
            engine.process(event(keysym::SHIFT_L, 42, SHIFT_MASK | RELEASE_MASK)),
            (false, Vec::new())
        );
    }

    #[test]
    fn roman_printable_uses_colemak_and_shortcut_passes_through() {
        let mut engine = LisleEngine::default();
        assert_eq!(
            engine.process(event(b'e' as u32, 18, 0)),
            (true, vec![Action::Commit("f".into())])
        );
        for modifier in [
            CONTROL_MASK,
            MOD1_MASK,
            MOD4_MASK,
            SUPER_MASK,
            HYPER_MASK,
            META_MASK,
        ] {
            assert_eq!(
                engine.process(event(b'f' as u32, 18, modifier)),
                (false, Vec::new())
            );
            assert_eq!(
                engine.process(event(b'f' as u32, 18, modifier | RELEASE_MASK)),
                (false, Vec::new())
            );
        }
    }

    #[test]
    fn num_lock_is_ignored_for_text_and_shift_selection() {
        let mut engine = LisleEngine::default();
        assert_eq!(
            engine.process(event(b'e' as u32, 18, MOD2_MASK)),
            (true, vec![Action::Commit("f".into())])
        );
        engine.process(event(keysym::SHIFT_R, 54, SHIFT_MASK | MOD2_MASK));
        engine.process(event(
            keysym::SHIFT_R,
            54,
            SHIFT_MASK | MOD2_MASK | RELEASE_MASK,
        ));
        assert_eq!(engine.state(), InputState::Hangul);
        assert_eq!(
            engine.process(event(b'k' as u32, 37, MOD2_MASK)),
            (true, vec![Action::Preedit("ㄱ".into())])
        );

        let state = CONTROL_MASK | MOD2_MASK;
        assert_eq!(
            engine.process(event(b'e' as u32, 18, state)),
            (
                false,
                vec![Action::Commit("ㄱ".into()), Action::Preedit(String::new())]
            )
        );
    }

    #[test]
    fn route_changing_repeats_use_current_semantics_and_release_original_routes() {
        let mut engine = LisleEngine::default();
        assert_eq!(
            engine.process(event(b'f' as u32, 18, CONTROL_MASK)),
            (false, Vec::new())
        );
        assert_eq!(
            engine.process(event(b'e' as u32, 18, 0)),
            (true, vec![Action::Commit("f".into())])
        );
        assert_eq!(
            engine.process(event(b'e' as u32, 18, RELEASE_MASK)),
            (false, Vec::new())
        );

        select_hangul(&mut engine);
        engine.process(event(b'k' as u32, 37, 0));
        assert_eq!(
            engine.process(event(keysym::BACK_SPACE, 14, 0)),
            (true, vec![Action::Preedit(String::new())])
        );
        assert_eq!(
            engine.process(event(keysym::BACK_SPACE, 14, 0)),
            (false, Vec::new())
        );
        assert_eq!(
            engine.process(event(keysym::BACK_SPACE, 14, RELEASE_MASK)),
            (false, Vec::new())
        );
    }

    #[test]
    fn repeated_shortcut_routes_keep_the_release_passthrough() {
        const MOD5: u32 = 1 << 7;
        let mut engine = LisleEngine::default();
        engine.process(event(b'f' as u32, 18, CONTROL_MASK));
        assert_eq!(
            engine.process(event(b'e' as u32, 18, MOD5)),
            (false, Vec::new())
        );
        let release_state = MOD5 | RELEASE_MASK;
        assert_eq!(
            engine.process(event(b'e' as u32, 18, release_state)),
            (false, Vec::new())
        );
    }

    #[test]
    fn shifted_shortcut_press_and_release_pass_through() {
        let mut engine = LisleEngine::default();
        let press_state = CONTROL_MASK | SHIFT_MASK;
        assert_eq!(
            engine.process(event(b':' as u32, 25, press_state)),
            (false, Vec::new())
        );
        let release_state = CONTROL_MASK | RELEASE_MASK;
        assert_eq!(
            engine.process(event(b';' as u32, 25, release_state)),
            (false, Vec::new())
        );
    }

    #[test]
    fn hangul_preedit_flushes_before_pass_through_boundaries() {
        let mut engine = LisleEngine::default();
        select_hangul(&mut engine);
        assert_eq!(
            engine.process(event(b'k' as u32, 37, 0)),
            (true, vec![Action::Preedit("ㄱ".into())])
        );
        assert_eq!(
            engine.process(event(b'f' as u32, 33, 0)),
            (true, vec![Action::Preedit("가".into())])
        );
        assert_eq!(
            engine.process(event(keysym::LEFT, 105, 0)),
            (
                false,
                vec![Action::Commit("가".into()), Action::Preedit(String::new())]
            )
        );
    }

    #[test]
    fn backspace_consumes_only_active_source_key_and_paired_release() {
        let mut engine = LisleEngine::default();
        select_hangul(&mut engine);
        engine.process(event(b'k' as u32, 37, 0));
        engine.process(event(b'f' as u32, 33, 0));
        assert_eq!(
            engine.process(event(keysym::BACK_SPACE, 14, 0)),
            (true, vec![Action::Preedit("ㄱ".into())])
        );
        assert_eq!(
            engine.process(event(keysym::BACK_SPACE, 14, RELEASE_MASK)),
            (true, Vec::new())
        );
        engine.process(event(keysym::BACK_SPACE, 14, 0));
        assert_eq!(
            engine.process(event(keysym::BACK_SPACE, 14, 0)),
            (false, Vec::new())
        );
    }

    #[test]
    fn escape_flushes_selects_roman_and_passes_through() {
        let mut engine = LisleEngine::default();
        select_hangul(&mut engine);
        engine.process(event(b'k' as u32, 37, 0));
        engine.process(event(b'f' as u32, 33, 0));
        let result = engine.process(event(keysym::ESCAPE, 1, 0));
        assert_eq!(engine.state(), InputState::Roman);
        assert_eq!(
            result,
            (
                false,
                vec![Action::Commit("가".into()), Action::Preedit(String::new())]
            )
        );
    }

    #[test]
    fn shortcut_flushes_before_passing_through() {
        let mut engine = LisleEngine::default();
        select_hangul(&mut engine);
        engine.process(event(b'k' as u32, 37, 0));
        engine.process(event(b'f' as u32, 33, 0));
        assert_eq!(
            engine.process(event(b'f' as u32, 18, CONTROL_MASK)),
            (
                false,
                vec![Action::Commit("가".into()), Action::Preedit(String::new())]
            )
        );
    }

    #[test]
    fn unknown_modifier_and_dead_key_are_not_consumed_as_text() {
        const MOD5: u32 = 1 << 7;
        let mut engine = LisleEngine::default();
        assert_eq!(
            engine.process(event(b'e' as u32, 18, MOD5)),
            (false, Vec::new())
        );
        assert_eq!(engine.process(event(0xfe51, 40, 0)), (false, Vec::new()));
    }

    #[test]
    fn host_dead_and_compose_sequences_remain_passthrough() {
        let mut engine = LisleEngine::default();
        for input in [
            event(0xfe51, 40, 0),
            event(0xfe51, 40, RELEASE_MASK),
            event(b'e' as u32, 18, 0),
            event(b'e' as u32, 18, RELEASE_MASK),
        ] {
            assert_eq!(engine.process(input), (false, Vec::new()));
        }
        assert_eq!(
            engine.process(event(b'e' as u32, 18, 0)),
            (true, vec![Action::Commit("f".into())])
        );

        let mut engine = LisleEngine::default();
        for input in [
            event(keysym::MULTI_KEY, 127, 0),
            event(keysym::MULTI_KEY, 127, RELEASE_MASK),
            event(b'\'' as u32, 40, 0),
            event(b'\'' as u32, 40, RELEASE_MASK),
            event(b'e' as u32, 18, 0),
            event(b'e' as u32, 18, RELEASE_MASK),
        ] {
            assert_eq!(engine.process(input), (false, Vec::new()));
        }
        assert_eq!(
            engine.process(event(b'e' as u32, 18, 0)),
            (true, vec![Action::Commit("f".into())])
        );
    }

    #[test]
    fn host_compose_tracks_arbitrary_length_until_xkbcommon_finishes() {
        let mut engine = LisleEngine::default();
        for input in [
            event(keysym::MULTI_KEY, 127, 0),
            event(keysym::MULTI_KEY, 127, RELEASE_MASK),
            event(b'-' as u32, 12, 0),
            event(b'-' as u32, 12, RELEASE_MASK),
            event(b'-' as u32, 12, 0),
            event(b'-' as u32, 12, RELEASE_MASK),
            event(b'.' as u32, 52, 0),
            event(b'.' as u32, 52, RELEASE_MASK),
        ] {
            assert_eq!(engine.process(input), (false, Vec::new()));
        }
        assert_eq!(
            engine.process(event(b'e' as u32, 18, 0)),
            (true, vec![Action::Commit("f".into())])
        );
    }

    #[test]
    fn escape_during_host_compose_selects_roman() {
        let mut engine = LisleEngine::default();
        select_hangul(&mut engine);
        assert_eq!(
            engine.process(event(keysym::MULTI_KEY, 127, 0)),
            (false, Vec::new())
        );
        assert_eq!(
            engine.process(event(keysym::ESCAPE, 1, 0)),
            (false, Vec::new())
        );
        assert_eq!(engine.state(), InputState::Roman);
    }

    #[test]
    fn reset_clears_composition_but_preserves_mode() {
        let mut engine = LisleEngine::default();
        select_hangul(&mut engine);
        engine.process(event(b'k' as u32, 37, 0));
        engine.reset();
        assert_eq!(engine.state(), InputState::Hangul);
        assert_eq!(
            engine.process(event(keysym::BACK_SPACE, 14, 0)),
            (false, Vec::new())
        );
        engine.focus_in();
        assert_eq!(engine.state(), InputState::Roman);
    }

    #[test]
    fn reset_preserves_held_key_release_identity() {
        let mut engine = LisleEngine::default();
        engine.process(event(b';' as u32, 25, CONTROL_MASK));
        engine.reset();
        assert_eq!(
            engine.process(event(b';' as u32, 25, CONTROL_MASK | RELEASE_MASK)),
            (false, Vec::new())
        );
    }

    #[test]
    fn every_host_action_flushes_then_passes_through() {
        let host_actions = [
            keysym::DELETE,
            keysym::LEFT,
            keysym::RIGHT,
            keysym::UP,
            keysym::DOWN,
            keysym::HOME,
            keysym::END,
            keysym::PAGE_UP,
            keysym::PAGE_DOWN,
            keysym::INSERT,
            keysym::TAB,
            keysym::RETURN,
            keysym::F1,
            keysym::F1 + 1,
            keysym::F1 + 2,
            keysym::F1 + 3,
            keysym::F1 + 4,
            keysym::F1 + 5,
            keysym::F1 + 6,
            keysym::F1 + 7,
            keysym::F1 + 8,
            keysym::F1 + 9,
            keysym::F1 + 10,
            keysym::F12,
        ];
        for keyval in host_actions {
            let mut engine = LisleEngine::default();
            select_hangul(&mut engine);
            engine.process(event(b'k' as u32, 37, 0));
            engine.process(event(b'f' as u32, 33, 0));
            assert_eq!(
                engine.process(event(keyval, 200, 0)),
                (
                    false,
                    vec![Action::Commit("가".into()), Action::Preedit(String::new())]
                ),
                "keyval={keyval:#x}"
            );
        }
    }
}
