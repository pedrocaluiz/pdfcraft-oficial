@echo off
if exist "C:\Sistemas\c123710\pdfcraft\pdfcraft.exe" (
    start "" "C:\Sistemas\c123710\pdfcraft\pdfcraft.exe" %*
) else if exist "C:\Sistemas\c123710\.cargo\target\pdfcraft\release\pdfcraft.exe" (
    start "" "C:\Sistemas\c123710\.cargo\target\pdfcraft\release\pdfcraft.exe" %*
) else (
    start "" "%~dp0target\release\pdfcraft.exe" %*
)
