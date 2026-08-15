{
  clippy,
  lib,
  libxml2,
  libxkbcommon,
  rustPlatform,
  stdenv,
}:

let
  cargoToml = lib.importTOML ../Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  version = cargoToml.package.version;

  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../LICENSE
      ../NOTICE
      ../README.md
      ../data
      ../docs
      ../src
      ../tests/core_contract.rs
      ../tests/browser
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  buildInputs = [ libxkbcommon ];

  nativeCheckInputs = [ clippy ];
  preCheck = ''
    cargo clippy \
      --all-targets \
      --all-features \
      --no-deps \
      --target ${stdenv.hostPlatform.rust.rustcTargetSpec} \
      --profile release \
      --offline \
      -- \
      --deny warnings
  '';

  postInstall = ''
    mkdir -p "$out/libexec" "$out/share/ibus/component" "$out/share/doc/lisle"
    mv "$out/bin/lisle" "$out/libexec/ibus-engine-lisle"
    rmdir "$out/bin"

    install -Dm644 ${../data/lisle.svg} \
      "$out/share/icons/hicolor/scalable/apps/lisle.svg"
    substitute ${../data/lisle.xml.in} \
      "$out/share/ibus/component/lisle.xml" \
      --replace-fail @EXECUTABLE@ "$out/libexec/ibus-engine-lisle" \
      --replace-fail @VERSION@ "${cargoToml.package.version}" \
      --replace-fail @ICON@ "$out/share/icons/hicolor/scalable/apps/lisle.svg"
    install -Dm644 ${../LICENSE} "$out/share/doc/lisle/LICENSE"
    install -Dm644 ${../NOTICE} "$out/share/doc/lisle/NOTICE"
    install -Dm644 ${../README.md} "$out/share/doc/lisle/README.md"
    install -Dm644 ${../docs/spec.md} "$out/share/doc/lisle/docs/spec.md"
    install -Dm644 ${../docs/implementation.md} \
      "$out/share/doc/lisle/docs/implementation.md"
    install -Dm644 ${../tests/browser/README.md} \
      "$out/share/doc/lisle/tests/browser/README.md"
    install -Dm644 ${../tests/browser/index.html} \
      "$out/share/doc/lisle/tests/browser/index.html"
    install -Dm644 ${../tests/browser/recorder.js} \
      "$out/share/doc/lisle/tests/browser/recorder.js"
  '';

  doInstallCheck = true;
  nativeInstallCheckInputs = [ libxml2 ];
  installCheckPhase = ''
    runHook preInstallCheck

    test -x "$out/libexec/ibus-engine-lisle"
    test ! -e "$out/bin"
    test -r "$out/share/icons/hicolor/scalable/apps/lisle.svg"
    test -r "$out/share/doc/lisle/LICENSE"
    test -r "$out/share/doc/lisle/NOTICE"
    test -r "$out/share/doc/lisle/docs/spec.md"
    test -r "$out/share/doc/lisle/docs/implementation.md"
    test -r "$out/share/doc/lisle/tests/browser/index.html"

    component="$out/share/ibus/component/lisle.xml"
    xmllint --noout "$component"
    grep -F "<exec>$out/libexec/ibus-engine-lisle --ibus</exec>" "$component"
    grep -F "<version>${cargoToml.package.version}</version>" "$component"
    grep -F "<icon>$out/share/icons/hicolor/scalable/apps/lisle.svg</icon>" "$component"
    grep -F "<layout>us+colemak</layout>" "$component"
    if grep -F '@' "$component"; then
      echo "unsubstituted placeholder in $component" >&2
      exit 1
    fi

    runHook postInstallCheck
  '';

  meta = {
    description = cargoToml.package.description;
    homepage = cargoToml.package.repository;
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
    isIbusEngine = true;
  };
}
