# Lisle implementation notes

## Reference integration path

Lisle의 초기 설계와 브라우저 수동 검증은 다음 경로를 기준으로 한다.

```text
Chromium native Wayland
  -> zwp_text_input_v3
  -> Mutter and GNOME Shell
  -> IBus input context
  -> Lisle zbus engine
  -> deterministic Rust state machine
```

IBus engine은 incoming `keycode`를 Linux evdev code로 해석한다. GNOME Shell은
Clutter keycode에서 8을 빼 IBus에 전달하고, forwarded event에는 다시 8을
더한다. Lisle 자체는 XKB offset을 더하거나 빼지 않는다.

## Process and object model

component XML이 `org.freedesktop.IBus.Lisle` process를 활성화한다. process는 private
IBus bus에 연결하고 `/org/freedesktop/IBus/Factory`를 export한다. `CreateEngine`마다
새 `/org/freedesktop/IBus/Engine/N` 객체와 독립된 `LisleEngine` 상태를 만든다.

각 Factory와 Engine object는 `org.freedesktop.IBus.Service.Destroy`도 제공한다.
Engine destroy 뒤 D-Bus object와 모든 transient composition state를 제거한다.

## Event delivery

- 변환하지 않는 host event: 필요한 Flush signal을 먼저 보낸 뒤
  `ProcessKeyEvent=false`를 반환한다.
- 로마자 printable과 Space: component의 `us(colemak)` XKB keymap이 해석한 원본
  press/release event를 `ProcessKeyEvent=false`로 통과시킨다. Commit, preedit,
  `ForwardKeyEvent` signal을 만들지 않는다.
- 한글 printable 또는 jamo: commit/preedit signal을 보내고 `true`를 반환한다.
- Colemak shortcut: component의 `us(colemak)` XKB keymap이 해석한 원본 event를
  `ProcessKeyEvent=false`로 통과시킨다. `ForwardKeyEvent`로 합성하지 않는다.
- consume한 press의 release도 consume한다.
- `ForwardKeyEvent`를 보낸 event에 대해 동시에 `false`를 반환하지 않는다.

한글 배열은 XKB keysym이 아니라 evdev keycode로 결정한다. 로마자 문자와 host
shortcut은 component의 Colemak XKB layout이 정한 keysym을 그대로 사용한다.

GNOME Shell 50.4는 IBus의 표준 `layout-variant` property 대신 존재하지 않는 `variant`
property를 읽는다. 따라서 split descriptor인 `layout=us`,
`layout_variant=colemak`은 `us`로 잘못 적용된다. 이 bridge에서 배열이 결정적으로
적용되도록 Lisle은 component의 `layout`에 GNOME XKB ID `us+colemak`을 직접 기록한다.
GNOME/Mutter의 synthetic `ForwardKeyEvent`는 raw modifier state를 보존하지 않으므로
modifier shortcut 변환에는 사용하지 않는다.

따라서 Caps/Lock 의미도 별도로 합성하지 않고 표준 `us(colemak)` XKB 동작을 따른다.
이 배열에서 실제 Caps Lock 위치는 `BackSpace`이고, Lisle은 그 event를 일반 host
Backspace로 통과시킨다.

Shift tap 후보의 press는 결과가 결정될 때까지 보류한다. host에 전달해야 하는
다른 event가 후보를 취소하면 Shift press를 먼저 replay한다. 로마자 shifted
printable에서는 이 replay 뒤 현재 문자 press와 두 release를 모두 통과시키므로
오른쪽 Shift를 문자 modifier로 사용해도 한글 상태를 선택하지 않는다. 한글 shifted
printable이 Lisle에서 소비되면 bare Shift event는 replay하지 않는다.

## Preedit lifecycle

활성 preedit은 `IBUS_ENGINE_PREEDIT_COMMIT`, 빈 preedit은
`IBUS_ENGINE_PREEDIT_CLEAR` mode로 보낸다. GNOME/Mutter는 click, focus transition,
input-source transition에서 engine callback 전에 cached COMMIT preedit을 이전
context에 반영한다. 이 검증 경로에서 context transition의 commit은 Mutter가
담당하며, Lisle은 callback을 받은 뒤 local state만 비운다.

한글 상태의 명시적인 printable 경계에서 Flush할 때는 빈 CLEAR preedit을 먼저 보내
조합 범위를 닫고, visible text와 경계 문자를 하나의 `CommitText`로 보낸다. Mutter는
같은 키 처리에서 발생한 IM event를 하나의 text-input-v3 `done`으로 묶는다. 이 배치 안의
여러 `commit_string`은 누적되지 않으므로 `CommitText("녕")`, `CommitText(" ")`처럼
나누면 Chromium에는 마지막 공백만 남는다. 새 조합이 즉시 이어지는 음절 경계는
별도 키 처리이므로 기존 text를 commit한 뒤 새 preedit을 보낸다.

| Callback | Lisle local action | Outbound text |
|---|---|---|
| `FocusIn`, `Enable` | empty Roman context | empty CLEAR preedit |
| `Reset` | clear composition and Shift gesture, preserve mode | empty CLEAR preedit |
| `FocusOut`, `Disable` | discard context and select Roman | none |
| `Destroy` | discard context and remove object | none |

Lifecycle callback에서 Lisle이 같은 text를 다시 commit하지 않는다. commit 성공
여부가 불명확할 때 재전송하지 않는다는 [`spec.md`](spec.md)의 안전 우선순위를
따른다. `FocusOut`이나 `Disable`에서 Lisle도 commit하면 Mutter가 이미 반영한 text가
중복될 수 있다.

키 처리 중 outbound preedit, commit 또는 `ForwardKeyEvent` signal 전송이 실패하면
local `LisleEngine::reset()`을 호출하고 오류를 반환한다. 이 오류 경로에서는 빈
preedit을 포함한 복구 signal을 추가로 보내지 않는다. 일부 signal이 이미 전달되었을
수 있으므로 text를 추측하여 다시 보내지 않는다.

Chromium text-input-v3의 local `CancelComposition()`은 Wayland request를 보내지
않으므로 engine에서 외부 cancel로 관찰할 수 없다. 이는 구현으로 추측하지 않고
known platform limitation으로 유지한다.

Chromium text-input-v3는 한 `done`에서 commit을 적용한 뒤에도 새 preedit이 직전에
적용한 preedit과 같으면 `OnPreeditString()`을 호출하지 않는다. Lisle에서 결합되지
않는 동일 자모를 반복하면 각 경계는 정상적으로 다음처럼 표현된다.

```text
첫 ㅋ: Preedit("ㅋ")
둘째 ㅋ: Commit("ㅋ"), Preedit("ㅋ")
```

둘째 입력의 commit은 이전 조합을 끝내지만 Chromium의 동일 문자열 검사는 그 사실을
고려하지 않는다. 따라서 새 `ㅋ` 조합이 Chromium에 전달되지 않고, `ㅋㅋㅋ` 또는
`ㅠㅠㅠ` 입력 직후 화면과 DOM 조합 상태가 한 자모 뒤처질 수 있다.

Lisle은 이를 우회하려고 여러 독립 자모를 하나의 커지는 preedit으로 유지하거나
commit을 지연하지 않는다. 그런 처리는 Chromium의 중복 preedit 억제 정책 위에
추가 추측을 쌓고 다른 client에서의 조합 범위와 commit 시점을 바꾸기 때문이다.
Lisle은 protocol에 맞는 `CommitText`와 `UpdatePreeditText`를 그대로 보내며 이 현상을
known Chromium limitation으로 취급한다.

## Component discovery

IBus 1.5.34는 `IBUS_COMPONENT_PATH`가 없으면 compile-time component directory만
scan한다. `$XDG_DATA_HOME/ibus/component` scan 코드는 upstream에서 비활성화되어
있다. NixOS에서는 `i18n.inputMethod.ibus.engines`가 `ibus-with-plugins` aggregate를
구성하고 공식 IBus module이 daemon, systemd/D-Bus, dconf 기반과 session 변수를 함께
관리한다. Lisle NixOS module은 이 목록에 Lisle package 하나만 기여한다. 다른 engine도
같은 `i18n.inputMethod.ibus.engines` 목록에 추가해야 하며, package나 component XML만
profile에 추가하는 것으로는 충분하지 않다. 사용자별 GNOME 입력 소스 선호는 이
runtime 계층과 별도로 dconf에서 관리한다.

## Primary upstream references

- [IBus Engine ABI](https://github.com/ibus/ibus/blob/1f7af28437afd62a6d145bfc81035e698a37411d/src/ibusengine.c)
- [IBus Service ABI](https://github.com/ibus/ibus/blob/1f7af28437afd62a6d145bfc81035e698a37411d/src/ibusservice.c)
- [IBus component XML](https://github.com/ibus/ibus/wiki/DevXML)
- [GNOME Shell input method bridge](https://github.com/GNOME/gnome-shell/blob/dcda6594b153aa179d92cc62e2414d84a43ab82c/js/misc/inputMethod.js)
- [Mutter preedit reset](https://github.com/GNOME/mutter/blob/8fe247a25a5b773e506c3f5f442ca0b7e3d5dc97/clutter/clutter/clutter-input-focus.c#L108-L128)
- [Chromium Wayland text-input-v3 feature](https://github.com/chromium/chromium/blob/bf0a91d23cfe3dd09db10104ea7f5b9c4621c5fe/ui/base/ui_base_features.cc#L137-L144)
- [Chromium text-input-v3 event application](https://github.com/chromium/chromium/blob/main/ui/ozone/platform/wayland/host/zwp_text_input_v3.cc)
- [nixpkgs buildRustPackage](https://github.com/NixOS/nixpkgs/tree/master/pkgs/build-support/rust/build-rust-package)
- [nixpkgs ibus-with-plugins](https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/ib/ibus-with-plugins/package.nix)
