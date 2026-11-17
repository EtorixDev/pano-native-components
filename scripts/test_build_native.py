from pathlib import Path
import tempfile
import unittest

from build_native import stage_artifacts


class NativeArtifactStagingTest(unittest.TestCase):
    def fixture(self, windows: bool, layout: str):
        root = Path(tempfile.mkdtemp(prefix="pano-native-artifacts-"))
        package_id = "path+file:///workspace/native-webview#0.1.0"
        output = root / layout
        output.mkdir(parents=True)
        messages = [{"reason": "build-script-executed", "package_id": package_id, "out_dir": str(output)}]
        names = ("pano_native_components", "native_webview")
        for name in names:
            filename = f"{name}.dll" if windows else f"lib{name}.so"
            source = root / "custom-target" / "custom-profile" / filename
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_bytes(name.encode())
            messages.append({
                "reason": "compiler-artifact", "package_id": package_id if name == "native_webview" else "pano",
                "target": {"name": name, "kind": ["cdylib"]}, "filenames": [str(source)], "fresh": True,
            })
        if windows:
            (output / "WebView2Loader.dll").write_bytes(b"loader")
        return root, output, messages

    def test_windows_stages_loader_from_old_and_new_layouts(self):
        for layout in ("target/release/build/native-webview-hash/out", "separate-build/release/build/native-webview/hash/out"):
            with self.subTest(layout=layout):
                root, _, messages = self.fixture(True, layout)
                destination = root / "staged"
                paths = stage_artifacts(list(reversed(messages)), destination)
                self.assertEqual({p.name for p in paths}, {"pano_native_components.dll", "native_webview.dll", "WebView2Loader.dll"})
                self.assertEqual((destination / "WebView2Loader.dll").read_bytes(), b"loader")

    def test_windows_rejects_missing_or_empty_loader_before_staging(self):
        for missing in (True, False):
            with self.subTest(missing=missing):
                root, output, messages = self.fixture(True, "arbitrary/build-output")
                loader = output / "WebView2Loader.dll"
                if missing:
                    loader.rename(output / "saved-loader.dll")
                else:
                    loader.write_bytes(b"")
                with self.assertRaisesRegex(ValueError, "Required runtime library is missing or empty"):
                    stage_artifacts(messages, root / "staged")
                self.assertFalse((root / "staged").exists())

    def test_linux_stages_both_libraries_without_a_windows_loader(self):
        root, _, messages = self.fixture(False, "build-output")
        paths = stage_artifacts(messages[1:], root / "staged")
        self.assertEqual({p.name for p in paths}, {"libpano_native_components.so", "libnative_webview.so"})
        self.assertEqual({p.read_bytes() for p in paths}, {b"pano_native_components", b"native_webview"})


if __name__ == "__main__":
    unittest.main()
