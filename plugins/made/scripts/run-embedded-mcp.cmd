@echo off
rem MADE plugin launcher for Windows hosts. Mirrors run-embedded-mcp.sh:
rem the embedded backend needs a state file, and an explicit
rem MADE_MCP_REDB_PATH always wins over the per-user default.
setlocal

set "PLUGIN_ROOT=%~dp0.."
set "BINARY=%PLUGIN_ROOT%\bin\made-mcp.exe"

rem The release bundle ships bin\made-mcp.exe and keeps priority. An install
rem straight from the repository has no bin\ — that path is gitignored — so
rem fall back to an installed made-mcp on PATH rather than failing to start.
if not exist "%BINARY%" (
  for %%I in (made-mcp.exe) do set "BINARY=%%~$PATH:I"
)

if not defined BINARY goto :nobinary
if not exist "%BINARY%" goto :nobinary
goto :haveBinary

:nobinary
echo MADE plugin: no made-mcp executable found. 1>&2
echo MADE plugin: looked for %PLUGIN_ROOT%\bin\made-mcp.exe and made-mcp on PATH. 1>&2
echo MADE plugin: install one with "cargo install made-mcp", or install the plugin from a release package. 1>&2
exit /b 127

:haveBinary

set "MADE_MCP_BACKEND=embedded"

if "%MADE_MCP_REDB_PATH%"=="" (
  if "%LOCALAPPDATA%"=="" (
    set "USER_STATE_ROOT=%USERPROFILE%\.local\state"
    set "MADE_STATE_ROOT=%USERPROFILE%\.local\state\underpass-made"
  ) else (
    set "USER_STATE_ROOT=%LOCALAPPDATA%"
    set "MADE_STATE_ROOT=%LOCALAPPDATA%\underpass-made"
  )
  if not exist "%MADE_STATE_ROOT%" mkdir "%MADE_STATE_ROOT%"
  set "MADE_MCP_REDB_PATH=%MADE_STATE_ROOT%\ceremonies.redb"
  if not exist "%MADE_MCP_REDB_PATH%" if "%MADE_MCP_LEGACY_REDB_PATH%"=="" if exist "%USER_STATE_ROOT%\underpass-choreographer\ceremonies.redb" set "MADE_MCP_LEGACY_REDB_PATH=%USER_STATE_ROOT%\underpass-choreographer\ceremonies.redb"
)

"%BINARY%" %*
