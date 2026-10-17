@echo off
setlocal

set "PATH=%USERPROFILE%\.cargo\bin;C:\Program Files\nodejs;%PATH%"

cd /d "%~dp0apps\gui" || exit /b 1

if not exist node_modules (
    echo Installing npm dependencies...
    call npm install || exit /b 1
)

call npm run dev
