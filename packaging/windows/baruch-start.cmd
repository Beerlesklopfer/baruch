@echo off
rem SPDX-License-Identifier: AGPL-3.0-or-later
rem Baruch starten: lokaler Server (Fenster minimiert) und Oberflaeche im Browser.
rem Laeuft der Server schon, meldet der zweite Start nur den belegten Port und beendet sich.
setlocal
if "%BARUCH_LISTEN%"=="" set "BARUCH_LISTEN=127.0.0.1:8765"
start "Baruch" /min "%~dp0baruch-server.exe" serve --listen %BARUCH_LISTEN%
rem Kurz warten, bis der Server antwortet (der erste Start legt die Datenbank an).
timeout /t 4 /nobreak >nul
start "" "http://%BARUCH_LISTEN%/"
endlocal
