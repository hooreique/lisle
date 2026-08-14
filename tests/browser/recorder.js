(() => {
  "use strict";

  const eventLog = document.querySelector("#events");
  const targets = [...document.querySelectorAll(".ime-target")];
  const records = [];
  let sequence = 0;

  function targetText(target) {
    return "value" in target ? target.value : target.textContent;
  }

  function selection(target) {
    if ("selectionStart" in target) {
      return { start: target.selectionStart, end: target.selectionEnd };
    }
    const current = window.getSelection();
    if (!current || current.rangeCount === 0 || !target.contains(current.anchorNode)) {
      return null;
    }
    const range = current.getRangeAt(0);
    const start = range.cloneRange();
    start.selectNodeContents(target);
    start.setEnd(range.startContainer, range.startOffset);
    const end = range.cloneRange();
    end.selectNodeContents(target);
    end.setEnd(range.endContainer, range.endOffset);
    return { start: start.toString().length, end: end.toString().length };
  }

  function closestTarget(node) {
    return node instanceof Element ? node.closest(".ime-target") : null;
  }

  function record(event) {
    const target = closestTarget(event.target);
    if (!target && event.type !== "selectionchange") return;
    const active = target || closestTarget(document.activeElement);
    if (!active) return;

    records.push({
      sequence: ++sequence,
      time: performance.now(),
      type: event.type,
      target: active.id,
      key: event.key ?? null,
      code: event.code ?? null,
      location: event.location ?? null,
      repeat: event.repeat ?? null,
      modifiers: {
        shift: event.shiftKey ?? null,
        control: event.ctrlKey ?? null,
        alt: event.altKey ?? null,
        meta: event.metaKey ?? null,
      },
      data: event.data ?? null,
      inputType: event.inputType ?? null,
      isComposing: event.isComposing ?? null,
      text: targetText(active),
      selection: selection(active),
    });
    eventLog.textContent = records.slice(-160).map((entry) => JSON.stringify(entry)).join("\n");
    eventLog.scrollTop = eventLog.scrollHeight;
  }

  for (const type of [
    "keydown",
    "keyup",
    "compositionstart",
    "compositionupdate",
    "compositionend",
    "beforeinput",
    "input",
    "focus",
    "blur",
    "pointerdown",
    "selectionchange",
  ]) {
    document.addEventListener(type, record, true);
  }

  document.querySelector("#reset").addEventListener("click", () => {
    for (const target of targets) {
      if ("value" in target) target.value = "foo bar";
      else target.textContent = "foo bar";
    }
    records.length = 0;
    sequence = 0;
    eventLog.textContent = "";
  });

  document.querySelector("#export").addEventListener("click", () => {
    const blob = new Blob([JSON.stringify(window.lisleSnapshot(), null, 2)], {
      type: "application/json",
    });
    const link = document.createElement("a");
    link.href = URL.createObjectURL(blob);
    link.download = `lisle-ime-${Date.now()}.json`;
    link.click();
    URL.revokeObjectURL(link.href);
  });

  window.lisleSnapshot = () => ({
    userAgent: navigator.userAgent,
    records: [...records],
    targets: Object.fromEntries(targets.map((target) => [target.id, targetText(target)])),
  });
})();
