@echo off
setlocal
set "FILE=%~1"
if "%FILE%"=="" (
  echo No file passed to signer. Skipping.
  exit /b 0
)

if defined HANDY_SIGN_WINDOWS_DISABLE (
  echo Signing disabled via HANDY_SIGN_WINDOWS_DISABLE. Skipping %FILE%.
  exit /b 0
)

echo No signing configured. Skipping signing for %FILE%.
exit /b 0
