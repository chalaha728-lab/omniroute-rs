@echo off
REM ============================================================
REM  OmniRoute-Rust Windows Quick Start
REM  Double-click this file to start the server.
REM  The first run auto-generates secrets — no .env needed.
REM ============================================================

echo.
echo  Starting OmniRoute-Rust...
echo  The browser will open automatically once the server is ready.
echo  Press Ctrl+C in this window to stop the server.
echo.

REM -- Start the server (it auto-generates secrets on first run) --
omniroute.exe

echo.
echo  Server stopped. Press any key to close this window.
pause >nul
