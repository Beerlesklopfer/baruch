@echo off
rem SPDX-License-Identifier: AGPL-3.0-or-later
rem Typst-Kommandozeile von Baruch: reicht alle Argumente an "baruch-server.exe typst" weiter
rem (eingebaute Engine, ohne Netz). Beispiel: baruch-typst compile buch.typ buch.pdf
rem Der Exit-Code ist der von baruch-server.exe (0 Erfolg, 1 Satz- oder Dateifehler, 2 falscher Aufruf).
"%~dp0baruch-server.exe" typst %*
exit /b %ERRORLEVEL%
