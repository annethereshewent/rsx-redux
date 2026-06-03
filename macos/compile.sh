rm -rf PSXMacEmulator
 ./build-rust.sh
 swift-bridge-cli create-package \
--bridges-dir ./generated \
--out-dir PSXMacEmulator \
--ios target/aarch64-apple-ios/release/librsx_redux_macos.a \
--simulator target/universal-ios/release/librsx_redux_macos.a \
--macos target/universal-macos/release/librsx_redux_macos.a \
--name PSXMacEmulator