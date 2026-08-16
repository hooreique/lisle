# Lisle

Lisle은 `x86_64-linux`에서 Linux evdev 물리 keycode를 고정 배열로 해석하는 IBus
입력기이다. 하나의 GNOME 입력 소스 안에서 다음 두 상태를 제공한다.

- 오른쪽 Shift 단일 탭: Cole Sebeol 한글 상태
- 왼쪽 Shift 단일 탭: Colemak 로마자 상태

이 프로젝트는 GNOME Wayland에서 Chromium을 사용하려다 `kime`를 함께 사용할 수
없다는 것을 알게 되어 시작했다.

현재 XKB 배열과 무관하게 Linux evdev 물리 keycode를 미국식 Qwerty 위치로
해석한다. 이 저장소에는 후보 창이나 설정 UI가 없다. 입력 동작은
[`docs/spec.md`](docs/spec.md)가 규정한다.

## Build

```sh
nix build .#default
```

결과에는 다음 파일이 설치된다.

```text
libexec/ibus-engine-lisle
share/ibus/component/lisle.xml
share/icons/hicolor/scalable/apps/lisle.svg
share/doc/lisle/{README.md,LICENSE,NOTICE}
share/doc/lisle/docs/{spec.md,implementation.md,nixos-integration.md}
share/doc/lisle/tests/browser/
```

`nix build`는 release build, test, Clippy, component XML 검증을 함께 수행한다.

## NixOS installation

NixOS module을 기본 설치 경로로 사용한다. NixOS의 공식
`i18n.inputMethod.ibus` 통합이 Lisle을 포함한 IBus aggregate, daemon, systemd user
unit, D-Bus service, dconf 기반과 session 변수를 일관된 경로로 구성한다.
Home Manager module을 두지 않고 이 책임을 NixOS에 모은 배경은
[`docs/nixos-integration.md`](docs/nixos-integration.md)에 기록한다.

```nix
# flake.nix의 inputs 안
inputs.lisle = {
  url = "github:hooreique/lisle";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

```nix
# nixosSystem 안
modules = [
  inputs.lisle.nixosModules.default
  ./configuration.nix
];
```

```nix
# configuration.nix
programs.lisle.enable = true;
```

`nixosModule` 단수 alias도 제공한다. module은 IBus를 기본 입력기로 활성화하고
Lisle package 하나를 `i18n.inputMethod.ibus.engines`에 기여한다. 다른 engine은
Lisle 설정이 아니라 같은 NixOS engine 목록에 추가한다.

```nix
i18n.inputMethod.ibus.engines =
  with pkgs.ibus-engines; [ hangul mozc ];
```

GNOME에서 Lisle을 표시하고 선택할지는 사용자별 dconf 선호다. Home Manager에서는
Lisle input이나 module을 추가하지 않고 기존 source 목록에 tuple만 직접 선언한다.

```nix
dconf.settings."org/gnome/desktop/input-sources".sources = [
  (lib.hm.gvariant.mkTuple [ "ibus" "lisle" ])
  (lib.hm.gvariant.mkTuple [ "xkb" "us" ])
];
```

적용은 NixOS, Home Manager 순서로 수행한 뒤 다시 로그인한다.

```sh
sudo nixos-rebuild switch
home-manager switch
# log out, then log in

systemctl --user show org.freedesktop.IBus.session.GNOME.service \
  --property=FragmentPath
ibus list-engine
```

`FragmentPath`는 `/etc/systemd/user/` 아래의 NixOS unit을 가리켜야 하고,
`ibus list-engine`에는 `lisle`이 보여야 한다. module은 engine package를 시스템
전체에 제공하지만 각 사용자의 GNOME source 목록은 자동으로 바꾸지 않는다.

## User-profile installation

이 flake는 module을 사용하지 않는 일반 Linux용으로 Lisle이 포함된 IBus aggregate도
제공한다.

```sh
nix profile add .#ibus-with-lisle
systemctl --user daemon-reload
systemctl --user restart org.freedesktop.IBus.session.GNOME.service
```

해당 user unit이 없는 배포판에서는 `ibus restart`를 사용한다. 그래도 목록에
나타나지 않으면 로그아웃 후 다시 로그인한다. NixOS의 `/etc/systemd/user` unit은
profile의 unit보다 우선하므로 NixOS에서는 이 절차 대신 위 NixOS module을 사용한다.
배포판 IBus와 Nix IBus aggregate를 동시에 실행하지 않는다.

일반 Linux의 기존 IBus를 유지하려면 Lisle package를 profile에 설치한 뒤
`IBUS_COMPONENT_PATH`에 Lisle의 `share/ibus/component`와 배포판의 기존 component
directory를 모두 넣어야 한다. 이 환경 변수는 기본 경로를 대체하므로 기존 경로를
누락해서는 안 된다.

## Chromium verification

브라우저 수동 검증 fixture는 Chromium의 native Wayland 경로를 사용한다.

```sh
chromium --ozone-platform=wayland tests/browser/index.html
```

검증 절차와 기대 결과는 [`tests/browser`](tests/browser)에 있다.

## Development

Rust toolchain은 항상 flake devShell 안에서 실행한다.

```sh
nix develop --command cargo test --all-targets
nix develop --command cargo clippy --all-targets --all-features -- --deny warnings
nix develop --command cargo fmt --all -- --check
nix flake check -L
```

IBus wire contract test는 private peer D-Bus connection에서 Factory, Engine,
Service interface와 serialized `IBusText`를 확인한다. 브라우저 수동 검증 fixture는
[`tests/browser`](tests/browser)에 있다.

## Known Chromium limitations

Chromium의 Wayland text-input-v3 구현은 내부 `CancelComposition()`을 compositor나
IBus에 알리지 않는다. 따라서 Lisle은 해당 내부 cancel과 아무 lifecycle 변화가
없는 경우를 구분할 수 없다. Lisle은 추측성 재전송보다 중복 입력과 다른 편집
문맥으로의 commit 방지를 우선한다. 자세한 lifecycle 정책과 upstream 근거는
[`docs/implementation.md`](docs/implementation.md)에 기록한다.

`ㅋㅋㅋ`, `ㅠㅠㅠ`처럼 결합되지 않는 동일 자모를 연속 입력하면 Chromium에서
마지막 자모의 새 preedit이 표시되지 않아 입력 직후 `ㅋㅋ`, `ㅠㅠ`만 보일 수 있다.
Lisle은 자모 경계에서 이전 preedit을 commit하고 다음 preedit을 시작하지만,
Chromium은 새 preedit 문자열이 직전 문자열과 같으면 commit 뒤에도 갱신을 생략한다.
이는 Chromium의 중복 preedit 억제 동작과 생기는 호환성 제한이다. Lisle은 이를
우회하려고 조합 범위나 commit 시점을 바꾸지 않는다.

## License

MIT. `data/lisle.svg`의 Hisle 출처와 배열 관련 고지는 [`NOTICE`](NOTICE)를
참조한다.
