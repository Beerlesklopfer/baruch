@echo off
rem SPDX-License-Identifier: AGPL-3.0-or-later
rem Baruch starten: lokaler Server (Fenster minimiert) und Oberflaeche im Browser.
rem Laeuft der Server schon, meldet der zweite Start nur den belegten Port und beendet sich.
rem
rem   BARUCH_LISTEN      Adresse des Servers (Vorgabe 127.0.0.1:8765)
rem   BARUCH_NO_BROWSER  auf 1 gesetzt: keinen Browser oeffnen
rem   BARUCH_BROWSER     app (Vorgabe): eigenes Fenster von Microsoft Edge (--app=URL), wenn msedge.exe
rem                      gefunden wird, sonst der Standardbrowser; standard: immer der Standardbrowser.
rem                      Im App-Fenster kann "Baruch beenden" das Fenster selbst schliessen.
rem
rem Beenden: in der Oberflaeche Baruch -> "Baruch beenden"; das minimierte Fenster des Servers
rem schliesst sich dann von selbst. Das Fenster zu schliessen beendet Baruch ebenfalls sauber.
setlocal
if "%BARUCH_LISTEN%"=="" set "BARUCH_LISTEN=127.0.0.1:8765"
set "BARUCH_URL=http://%BARUCH_LISTEN%/"
start "Baruch" /min "%~dp0baruch-server.exe" serve --listen %BARUCH_LISTEN%
if "%BARUCH_NO_BROWSER%"=="1" goto done
rem Kurz warten, bis der Server antwortet (der erste Start legt die Datenbank an).
timeout /t 4 /nobreak >nul
if /i "%BARUCH_BROWSER%"=="standard" goto standard
set "BARUCH_EDGE="
if exist "%LocalAppData%\Microsoft\Edge\Application\msedge.exe" set "BARUCH_EDGE=%LocalAppData%\Microsoft\Edge\Application\msedge.exe"
if exist "%ProgramFiles%\Microsoft\Edge\Application\msedge.exe" set "BARUCH_EDGE=%ProgramFiles%\Microsoft\Edge\Application\msedge.exe"
if exist "%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe" set "BARUCH_EDGE=%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe"
if not defined BARUCH_EDGE goto standard
start "" "%BARUCH_EDGE%" --app=%BARUCH_URL%
goto done
:standard
start "" "%BARUCH_URL%"
:done
endlocal
