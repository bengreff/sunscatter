@echo off
rem Run the game (Windows).
rem   run          the game (release build)
rem   run dev      faster rebuilds while iterating (Bevy dynamic linking; never for releases)
rem   run play     the 2026-10-01 low-orbit playtest save (own home dir; Ctrl+Alt+L at the pad)
cd /d "%~dp0"
if "%1"=="" goto release
if /i "%1"=="dev" goto dev
if /i "%1"=="play" goto play
echo unknown mode '%1' (modes: dev, play)
exit /b 2

:release
cargo run -p game --release
exit /b %ERRORLEVEL%

:dev
cargo run -p game --release --features dev
exit /b %ERRORLEVEL%

:play
set "SUNSCATTER_HOME=%TEMP%\sunscatter-play"
if not exist "%SUNSCATTER_HOME%\saves" mkdir "%SUNSCATTER_HOME%\saves"
copy /y docs\plans\playtest-2026-10-01-orbit.ron "%SUNSCATTER_HOME%\saves\quicksave.ron" >nul
echo Playtest save ready: Ctrl+Alt+L at the pad loads it (saves and settings in %SUNSCATTER_HOME%).
cargo run -p game --release
exit /b %ERRORLEVEL%
