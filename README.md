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
share/doc/lisle/docs/{spec.md,implementation.md}
share/doc/lisle/tests/browser/
```

`nix build`는 release build, test, Clippy, component XML 검증을 함께 수행한다.

## NixOS installation

Lisle 패키지만 `environment.systemPackages`에 추가해서는 IBus가 component를
발견하지 못한다. NixOS IBus module의 engine 목록에 추가해야 한다.

```nix
{
  i18n.inputMethod = {
    enable = true;
    type = "ibus";
    ibus.engines = [
      inputs.lisle.packages.${pkgs.stdenv.hostPlatform.system}.lisle
    ];
  };
}
```

적용 후 로그아웃하고 다시 로그인한 다음 GNOME Settings의 Keyboard, Input
Sources에서 `Lisle`을 추가한다.

## User-profile installation

이 flake는 Lisle이 포함된 IBus aggregate도 제공한다.

```sh
nix profile add .#ibus-with-lisle
systemctl --user daemon-reload
systemctl --user restart org.freedesktop.IBus.session.GNOME.service
```

해당 user unit이 없는 배포판에서는 `ibus restart`를 사용한다. 그래도 목록에
나타나지 않으면 로그아웃 후 다시 로그인한다. 배포판 IBus와 Nix IBus aggregate를
동시에 실행하지 않는다.

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
