# Native Components for Pano Scrobbler

A JNI lib, used for accessing some native APIs on desktop, that are not possible with JVM.

### Build

Build the runtime artifact bundle with Python 3 and Cargo:

```sh
python scripts/build_native.py --output-dir target/pano-artifacts --release
```

The helper stages both native libraries and the Windows WebView2 loader from
Cargo's reported paths. Cargo build options such as `--target` and `--profile`
can be passed to the helper. All required runtime libraries must exist and be
nonempty before the bundle is copied.

Linux may need distro-specific development dependencies for webkit6.

### Test

```sh
javac -h . PanoNativeComponents.java

python scripts/build_native.py --output-dir target/pano-artifacts --release && javac -d . PanoNativeComponents.java && java -Djava.library.path=target/pano-artifacts/ dev.etorix.panoscrobbler.PanoNativeComponents
```

or (if Powershell)

```sh
python scripts/build_native.py --output-dir target/pano-artifacts --release && javac -d . PanoNativeComponents.java && java "-Djava.library.path=target/pano-artifacts/" dev.etorix.panoscrobbler.PanoNativeComponents
```

### Package

```sh
cp target/pano-artifacts/pano_native_components.dll pano-scrobbler-dir/desktopApp/resources/windows-x64/
cp target/pano-artifacts/native_webview.dll pano-scrobbler-dir/desktopApp/resources/windows-x64/
cp target/pano-artifacts/WebView2Loader.dll pano-scrobbler-dir/desktopApp/resources/windows-x64/
```

```sh
cp target/pano-artifacts/libpano_native_components.so pano-scrobbler-dir/desktopApp/resources/linux-x64/
cp target/pano-artifacts/libnative_webview.so pano-scrobbler-dir/desktopApp/resources/linux-x64/
```


I used code from these projects as a reference:

https://github.com/Mange/mpris-rs

https://github.com/KDE/kdeconnect-kde


### License

SPDX-License-Identifier: GPL-3.0-or-later
