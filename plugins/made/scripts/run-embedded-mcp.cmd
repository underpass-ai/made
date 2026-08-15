@echo off
rem MADE plugin launcher for Windows hosts. Mirrors run-embedded-mcp.sh:
rem the embedded backend needs a state file, and an explicit
rem MADE_MCP_REDB_PATH always wins over the per-user default.
setlocal

set "PLUGIN_ROOT=%~dp0.."
set "BINARY=%PLUGIN_ROOT%\bin\made-mcp.exe"

if not exist "%BINARY%" (
  echo MADE plugin: missing executable %BINARY% 1>&2
  echo MADE plugin: build the local plugin bundle before installing it 1>&2
  exit /b 127
)

set "MADE_MCP_BACKEND=embedded"

if "%MADE_MCP_REDB_PATH%"=="" (
  if "%LOCALAPPDATA%"=="" (
    set "MADE_STATE_ROOT=%USERPROFILE%\.local\state\underpass-made"
  ) else (
    set "MADE_STATE_ROOT=%LOCALAPPDATA%\underpass-made"
  )
  if not exist "%MADE_STATE_ROOT%" mkdir "%MADE_STATE_ROOT%"
  set "MADE_MCP_REDB_PATH=%MADE_STATE_ROOT%\ceremonies.redb"
)

"%BINARY%" %*
