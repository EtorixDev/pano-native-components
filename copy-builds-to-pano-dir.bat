python "%~dp0scripts\build_native.py" --output-dir target/pano-artifacts/windows-x64 --release
if errorlevel 1 exit /b %errorlevel%
copy "%~dp0target\pano-artifacts\windows-x64\pano_native_components.dll" "%~dp0..\pano-scrobbler\desktopApp\resources\windows-x64\"
copy "%~dp0target\pano-artifacts\windows-x64\native_webview.dll" "%~dp0..\pano-scrobbler\desktopApp\resources\windows-x64\"
copy "%~dp0target\pano-artifacts\windows-x64\WebView2Loader.dll" "%~dp0..\pano-scrobbler\desktopApp\resources\windows-x64\"
