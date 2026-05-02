# RustDeskCE - A fork of RustDesk
This repo is a fork of [RustDesk](https://github.com/rustdesk/rustdesk), thanks for their excellent works!
<p align="center">
  <img src="res/logo-header.svg" alt="RustDesk - Customized Edition"><br>
</p>

> [!Caution]
> **Misuse Disclaimer:** <br>
> The developers of RustDesk **do not condone or support any unethical or illegal use of this software**. Misuse, such as unauthorized access, control or invasion of privacy, is strictly against our guidelines. The authors are not responsible for any misuse of the application.
> The customized edition is available to the public only for learning purposes. **Do not** use this edition for illegal activities.

## How to build
1. Fork or clone this repository, then make changes based on [Customizations.md](docs/Code_Implementation/Customizations.md), or modify it yourself.
2. Push your code to Github, make sure Github Actions has R/W permission.
3. In Github Actions, run Flutter Manually Build workflow.
4. You can also set up your own build environment; see [RustDesk](https://rustdesk.com/docs/en/dev/build/) offical documents for detailed instructions.

## Silent deployment guidiance

1. Download the latest MSI build.
2. Create a script named `setup.cmd`, place rustdesk-x.x.x-x86_64.msi and script in one directory.
3. Write contents below into script:
```bat
@echo off
setlocal enabledelayedexpansion
cd /d %~dp0
set "WorkDir=%~dp0"

REM Install RustDesk client
set "rustdeskMsi="
for %%f in ("%WorkDir%rustdesk-*.msi") do (
  if exist "%%f" (
    set "rustdeskMsi=%%f"
  ) else (
    echo RustDesk MSI package not found!
    echo Press any key to exit...
    pause >nul
    exit /b 1
  )
)

echo Installing RustDesk client: !rustdeskMsi!
msiexec /i "!rustdeskMsi!" /qr /norestart CREATESTARTMENUSHORTCUTS="1" CREATEDESKTOPSHORTCUTS="0" INSTALLPRINTER="0"
if !errorlevel! neq 0 (
    echo RustDesk client installation failed!
    pause
    exit /b 1
)
start "" "%ProgramFiles%\RustDesk\RustDesk.exe" --server
```
4. Run script with administrator privillege.
5. Use `LShift+LAlt+R` to show RustDesk main window

## Further Instructions
For anyone who wants to make a CE by himself, see [Customizations.md](docs/Code_Implementation/Customizations.md)