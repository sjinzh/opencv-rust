#!/bin/bash

set -vex

rm -rf "$VCPKG_INSTALLATION_ROOT"
export VCPKG_ROOT="$HOME/build/vcpkg"
if [[ -e "$VCPKG_ROOT" && ! -e "$VCPKG_ROOT/.git" ]]; then
	rm -rf "$VCPKG_ROOT"
fi
if [ ! -e "$VCPKG_ROOT" ]; then
	git clone --depth=1 https://github.com/Microsoft/vcpkg.git "$VCPKG_ROOT"
fi
pushd "$VCPKG_ROOT"
git fetch --all --prune --tags
git checkout .
git checkout 025e564979cc01d0fbc5c920aa8a36635efb01bb
./bootstrap-vcpkg.sh -disableMetrics
#./vcpkg integrate install
echo "set(VCPKG_BUILD_TYPE release)" >> "$VCPKG_ROOT/triplets/x64-windows.cmake"
echo "set(VCPKG_BUILD_TYPE release)" >> "$VCPKG_ROOT/triplets/x64-windows-static.cmake"
echo "set(VCPKG_BUILD_TYPE release)" >> "$VCPKG_ROOT/triplets/x86-windows.cmake"
export VCPKG_DEFAULT_TRIPLET=x64-windows
#./vcpkg install llvm  # takes very long time
choco install -y llvm --version 12.0.1
"$VCPKG_ROOT/vcpkg" upgrade --no-dry-run
"$VCPKG_ROOT/vcpkg" install --recurse "opencv${VCPKG_OPENCV_VERSION}[contrib,nonfree]"
popd
