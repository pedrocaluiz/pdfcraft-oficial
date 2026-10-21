# Compilar o PdfCraft no Mac (Apple Silicon, M1–M5)

Não dá para gerar o app de Mac a partir do Windows: o build precisa do SDK da Apple, do linker da Apple
e da assinatura (`codesign`), que só existem no macOS. Compile direto no Mac. O repositório já traz o
script de empacotamento (`packaging/macos/package.sh`).

## 1. Pré-requisitos (uma vez)

```bash
xcode-select --install                     # Command Line Tools (clang, SDK)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup target add aarch64-apple-darwin     # alvo nativo do M5
```

## 2. Obter o código

```bash
git clone <URL-do-seu-repositorio> pdfcraft && cd pdfcraft
```

## 3. Gerar o executável

**Só o binário (mais rápido)**

```bash
cargo build --release -p pdfcraft -p pdfcraft-cli
./target/release/pdfcraft caminho/arquivo.pdf
```

**App `PdfCraft.app` + `.dmg` para Apple Silicon (com ícone `pdfcraft.icns`)**

```bash
packaging/macos/package.sh --arch aarch64
# resultado em dist/: pdfcraft-<versão>-macos-aarch64.dmg e o zip do CLI
```

Sem certificado Apple, o script assina "ad-hoc". Funciona na sua máquina; em outro Mac o Gatekeeper
bloqueia (clique direito → Abrir, ou `xattr -dr com.apple.quarantine PdfCraft.app`).
Para distribuir, exporte `MACOS_SIGN_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` e `APPLE_TEAM_ID`
(veja o cabeçalho do `package.sh`).

## 4. Ícone no Dock

O ícone vem de `assets/app-icon/pdfcraft.icns`, embutido no `.app` pelo `package.sh`. Rodando o binário
solto (`target/release/pdfcraft`), o ícone do Dock é o PNG embutido no código.
