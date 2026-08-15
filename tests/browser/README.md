# Chromium GNOME Wayland smoke test

이 fixture는 실제 GNOME Wayland, IBus, Chromium 경로를 검증한다. DevTools의
`Input.dispatchKeyEvent`, Playwright `keyboard`, JavaScript synthetic event는 IBus를
통과하지 않으므로 사용하지 않는다.

```sh
chromium --ozone-platform=wayland tests/browser/index.html
```

GNOME 입력 소스를 Lisle로 선택한 뒤 물리 키보드로 다음을 확인한다. 표기의 영문은
미국식 Qwerty 물리 위치이다.

| Scenario | Input | Expected result |
|---|---|---|
| Initial Roman | `e` | `f` |
| Hangul | right Shift tap, `k f x`, Space | `각 ` |
| Escape | right Shift tap, `k f`, Escape, `e` | `가f` plus one host Escape |
| Crying vowel | right Shift tap, `i f a m r 2 j t b b`, Space | `망했어ㅜㅜ ` |
| Shortcut | right Shift tap, `k f`, Control+`e` | `가` then exactly one Control+`f` |
| Space/backspace | right Shift tap, `j f s h e a`, Space, Backspace | Backspace removes the new space |

다음 lifecycle case를 textarea, input, contenteditable에서 각각 수행한다.

1. Field A에서 right Shift tap 후 `k f`를 입력한다.
2. Field B를 mouse로 click한다.
3. `가`는 A에 한 번만 남고 B에는 나타나지 않아야 한다.
4. B에서 `e`를 입력하면 새 context의 Roman 출력 `f`가 나와야 한다.
5. 기존 text 전체를 선택하고 한글을 여러 음절 입력해 selection이 첫 commit에서만
   교체되고 이후 commit은 새 caret 뒤에 이어지는지 확인한다.
6. XKB 입력 소스를 Qwerty가 아닌 배열로 바꾼 뒤 같은 물리 위치가 같은 결과를
   내는지 확인한다.

3번에서 `가`가 A에 남는 것은 Lisle이 `FocusOut`에서 새 `CommitText`를 보내기
때문이 아니다. Mutter가 cached COMMIT preedit을 이전 문맥에 반영한 뒤 Lisle
callback이 local 상태를 폐기하는 통합 계약을 확인한다.

오른쪽의 event log와 `Export JSON` 결과에서 중복 `input`, 새 context로 이동한
preedit, stuck modifier가 없는지 확인한다. 위 표처럼 조합을 끝내는 경계 입력까지
마친 뒤에는 최종 text 전체가 정확히 일치해야 하며 부분 문자열만 일치하는 것은
성공으로 보지 않는다.

Chromium에서는 결합되지 않는 동일 자모를 반복 입력할 때 commit 뒤의 동일한 새
preedit을 생략하는 알려진 제한이 있다. 예를 들어 `ㅋㅋㅋ`, `ㅠㅠㅠ` 입력 직후,
아직 Space 같은 경계를 입력하기 전에는 마지막 자모가 표시되지 않을 수 있다. 이
중간 표시만으로 Lisle smoke test 실패를 판정하지 않는다. 경계 입력 뒤의 최종
text는 위 기준대로 정확해야 한다. Lisle에서는 Chromium 전용 commit/preedit
우회를 적용하지 않는다. 원인과 정책은
[`docs/implementation.md`](../../docs/implementation.md)에 기록되어 있다.
