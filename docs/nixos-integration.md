# NixOS IBus 통합 결정

Lisle은 `nixosModule`만 제공하고 Home Manager module은 제공하지 않는다. 이 문서는
그 이유와 설정의 책임 경계를 나중에 다시 확인하기 위한 짧은 기록이다.

## 결정

NixOS의 공식 `i18n.inputMethod.ibus` module이 IBus runtime 전체를 소유한다.
Lisle의 NixOS module은 다음 두 가지 일만 한다.

- IBus를 입력기로 활성화한다.
- Lisle package를 `i18n.inputMethod.ibus.engines`에 추가한다.

daemon, engine aggregate, systemd user unit, D-Bus service, GTK cache와 session 환경
변수는 NixOS가 함께 구성한다. 다른 IBus engine도 Lisle 전용 option이 아니라 같은
NixOS 목록에 추가한다.

```nix
programs.lisle.enable = true;

i18n.inputMethod.ibus.engines =
  with pkgs.ibus-engines; [ hangul mozc ];
```

Home Manager는 runtime에 관여하지 않고 사용자별 GNOME 입력 소스 선호만 관리한다.

```nix
dconf.settings."org/gnome/desktop/input-sources".sources = [
  (lib.hm.gvariant.mkTuple [ "ibus" "lisle" ])
  (lib.hm.gvariant.mkTuple [ "xkb" "us" ])
];
```

## Home Manager module을 없앤 이유

IBus는 engine package를 각각 설치한다고 자동으로 합쳐 주지 않는다. 실행 중인
daemon은 하나의 `ibus-with-plugins` aggregate에서 component 목록을 읽는다. 따라서
Home Manager가 Lisle용 IBus를 따로 만들면 다음 문제가 생긴다.

- NixOS와 Home Manager가 서로 다른 daemon, aggregate, systemd unit과 D-Bus
  activation을 만들게 된다.
- Home Manager unit이 NixOS의 `/etc/systemd/user` unit을 덮어쓰도록 별도 우선순위
  처리가 필요하다.
- 다른 engine 목록과 session 변수, GTK cache를 두 설정에서 중복 관리하게 된다.
- module을 제거하거나 소유권을 옮길 때 사용자 홈에 남은 unit과 service를 함께
  정리해야 한다.

Lisle은 engine 하나일 뿐 IBus runtime의 소유자가 아니다. 시스템 전체 engine 집합은
NixOS가 한 번 조립하고, 사용자가 GNOME 목록에서 무엇을 볼지는 Home Manager dconf로
분리하는 편이 단순하고 예측 가능하다.

## 기억할 책임 경계

| 설정 | 소유자 |
|---|---|
| IBus daemon과 aggregate | NixOS `i18n.inputMethod.ibus` |
| Lisle engine 등록 | Lisle `nixosModule` |
| 추가 IBus engine | NixOS `i18n.inputMethod.ibus.engines` |
| GNOME 입력 소스 목록과 순서 | 사용자/Home Manager dconf |

문제가 생기면 `systemctl --user show
org.freedesktop.IBus.session.GNOME.service --property=FragmentPath`가
`/etc/systemd/user/`의 unit을 가리키는지, `ibus list-engine`에 `lisle`이 있는지부터
확인한다.
