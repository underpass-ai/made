@echo off
rem MADE plugin launcher for Windows hosts. Mirrors run-embedded-mcp.sh:
rem the embedded backend needs a state file, and an explicit
rem MADE_MCP_STORE_PATH always wins over the per-user default.
setlocal

set "PLUGIN_ROOT=%~dp0.."

rem An explicit binary wins over everything below.
if not "%MADE_MCP_BIN%"=="" goto :explicitBinary
set "BINARY=%PLUGIN_ROOT%\bin\made-mcp.exe"
goto :bundledBinary

:explicitBinary
set "BINARY=%MADE_MCP_BIN%"
if not exist "%BINARY%" (
  echo MADE plugin: MADE_MCP_BIN is set to "%BINARY%", which does not exist. 1>&2
  exit /b 127
)
goto :haveBinary

:bundledBinary

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
echo MADE plugin: run /made:setup in Claude Code or the made-setup skill in Codex. 1>&2
exit /b 127

:haveBinary

set "MADE_MCP_BACKEND=embedded"

rem Flattened deliberately. cmd expands %VAR% for a whole parenthesised block
rem when it parses the block, so a variable set inside one reads back as its
rem *previous* value on the next line — which left this launcher pointing at
rem `\ceremonies.redb` at the drive root. Labels keep every read after its
rem write without depending on delayed expansion.
if not "%MADE_MCP_STORE_PATH%"=="" goto :havePath

rem The embedded backend refuses to start without a state file: where
rem ceremonies survive a restart is an operator decision. A plugin has no
rem operator to ask, so it picks the conventional per-user state directory.
if "%LOCALAPPDATA%"=="" goto :profileState
set "USER_STATE_ROOT=%LOCALAPPDATA%"
goto :haveStateRoot

:profileState
set "USER_STATE_ROOT=%USERPROFILE%\.local\state"

:haveStateRoot
set "MADE_STATE_ROOT=%USER_STATE_ROOT%\underpass-made"
if not exist "%MADE_STATE_ROOT%" mkdir "%MADE_STATE_ROOT%"
set "MADE_MCP_STORE_PATH=%MADE_STATE_ROOT%\ceremonies.sqlite3"
if not exist "%MADE_MCP_STORE_PATH%" if exist "%MADE_STATE_ROOT%\ceremonies.redb" goto :legacyStore
goto :havePath

:legacyStore
echo MADE plugin: legacy Redb store found at "%MADE_STATE_ROOT%\ceremonies.redb". 1>&2
echo MADE plugin: convert it before upgrading with made-mcp v0.2.0: 1>&2
echo MADE plugin:   made-mcp share-store "%MADE_STATE_ROOT%\ceremonies.redb" 1>&2
echo MADE plugin: the original is kept as a backup; no new store was created. 1>&2
exit /b 2

:havePath

rem No %* — the launcher starts the MCP server and nothing else. The binary
rem reads a leading argument as a maintenance command, so forwarding whatever
rem a host happened to pass would exit 2 instead of serving, and only here.
rem The POSIX launcher already drops them.
"%BINARY%"
