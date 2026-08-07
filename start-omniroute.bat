@echo off
REM ============================================================
REM  OmniRoute-Rust Windows Quick Start
REM  Double-click this file OR run it from PowerShell/CMD
REM ============================================================

REM -- Generate random secrets (you only need to do this once) --
REM -- If you already have secrets in .env, this will skip them --
if not exist .env (
  echo Generating secrets...
  powershell -Command "$jwt = -join ((48..122) | Get-Random -Count 48 | ForEach-Object {[char]$_}); $key = -join ((48..57)+(97..102) | Get-Random -Count 64 | ForEach-Object {[char]$_}); Set-Content -Path .env -Value \"JWT_SECRET=$jwt`nAPI_KEY_SECRET=$key`nINITIAL_PASSWORD=ChangeMe123!`nPORT=20128`nLOG_LEVEL=info`nOPENAI_API_KEY=sk-PUT-YOUR-REAL-KEY-HERE\""
  echo.
  echo ============================================
  echo  .env file created! Edit it to add your
  echo  OPENAI_API_KEY, then run this file again.
  echo ============================================
  pause
  exit /b
)

REM -- Start the server --
echo Starting OmniRoute-Rust...
echo Open http://localhost:20128 in your browser once it says "ready"
echo Press Ctrl+C to stop
echo.
omniroute.exe
pause
