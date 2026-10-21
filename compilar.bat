@echo off
rem Compila o PdfCraft (Release). Funciona em dois cenarios:
rem  - Maquina CAIXA: usa C:\Sistemas\c123710 (pasta liberada pelo AppLocker) e o SDK em ScopeCppSDK.
rem  - Maquina pessoal: usa o Rust padrao (%USERPROFILE%\.cargo) e as Build Tools do Visual Studio.
if exist "C:\Sistemas\c123710\.cargo\bin\cargo.exe" (
    set "CARGO_HOME=C:\Sistemas\c123710\.cargo"
    set "RUSTUP_HOME=C:\Sistemas\c123710\.rustup"
    set "CARGO_TARGET_DIR=C:\Sistemas\c123710\.cargo\target\pdfcraft"
    set "PATH=C:\Program Files\Microsoft Visual Studio\18\Professional\SDK\ScopeCppSDK\vc15\SDK\bin;C:\Program Files\Microsoft Visual Studio\18\Professional\SDK\ScopeCppSDK\vc15\VC\bin;C:\Sistemas\c123710\.cargo\bin;%PATH%"
    set "LIB=C:\Program Files\Microsoft Visual Studio\18\Professional\SDK\ScopeCppSDK\vc15\VC\lib;C:\Program Files\Microsoft Visual Studio\18\Professional\SDK\ScopeCppSDK\vc15\SDK\lib"
    set "INCLUDE=C:\Program Files\Microsoft Visual Studio\18\Professional\SDK\ScopeCppSDK\vc15\VC\include;C:\Program Files\Microsoft Visual Studio\18\Professional\SDK\ScopeCppSDK\vc15\SDK\include\ucrt;C:\Program Files\Microsoft Visual Studio\18\Professional\SDK\ScopeCppSDK\vc15\SDK\include\um;C:\Program Files\Microsoft Visual Studio\18\Professional\SDK\ScopeCppSDK\vc15\SDK\include\shared"
    set "RC_PATH=C:\Program Files\Microsoft Visual Studio\18\Professional\SDK\ScopeCppSDK\vc15\SDK\bin\rc.exe"
    set "BUILD_DIR=C:\Sistemas\c123710\.cargo\target\pdfcraft\release"
    set "OUT=C:\Sistemas\c123710\pdfcraft"
) else (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
    set "BUILD_DIR=%~dp0target\release"
    set "OUT=%~dp0target\release"
)

rem Falha (em vez de gerar um .exe sem icone) se o icone nao puder ser embutido.
set "PDFCRAFT_REQUIRE_WINRES=1"

rem Se o executavel estiver aberto, move para .old para evitar erro de acesso negado
if exist "%BUILD_DIR%\pdfcraft.exe" (
    del "%BUILD_DIR%\pdfcraft.exe" >nul 2>&1
    if exist "%BUILD_DIR%\pdfcraft.exe" move /y "%BUILD_DIR%\pdfcraft.exe" "%BUILD_DIR%\pdfcraft_old.exe" >nul 2>&1
)
if exist "%OUT%\pdfcraft.exe" (
    del "%OUT%\pdfcraft.exe" >nul 2>&1
    if exist "%OUT%\pdfcraft.exe" move /y "%OUT%\pdfcraft.exe" "%OUT%\pdfcraft_old.exe" >nul 2>&1
)

echo ========================================================
echo Compilando PdfCraft (Release)...
echo ========================================================
cd /d "%~dp0"
cargo build --release -p pdfcraft -p pdfcraft-cli

if %ERRORLEVEL% EQU 0 (
    echo.
    echo ========================================================
    echo Compilacao concluida com sucesso!

    if /i not "%BUILD_DIR%"=="%OUT%" (
        if not exist "%OUT%" mkdir "%OUT%"
        copy /y "%BUILD_DIR%\pdfcraft.exe" "%OUT%\pdfcraft.exe" >nul 2>&1
        copy /y "%BUILD_DIR%\pdfcraft-cli.exe" "%OUT%\pdfcraft-cli.exe" >nul 2>&1
        if exist "%BUILD_DIR%\pdfcraft.pdb" copy /y "%BUILD_DIR%\pdfcraft.pdb" "%OUT%\pdfcraft.pdb" >nul 2>&1
        if exist "%BUILD_DIR%\pdfcraft_cli.pdb" copy /y "%BUILD_DIR%\pdfcraft_cli.pdb" "%OUT%\pdfcraft_cli.pdb" >nul 2>&1
        echo Arquivos atualizados na subpasta do aplicativo:
    )

    echo Executavel GUI: %OUT%\pdfcraft.exe
    echo Executavel CLI: %OUT%\pdfcraft-cli.exe
    echo ========================================================
) else (
    echo.
    echo [ERRO] Falha na compilacao.
)
pause
