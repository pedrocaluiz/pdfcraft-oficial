# Guia de Instalação e Compilação do PdfCraft no Windows

Este guia explica como configurar o ambiente Rust e compilar o **PdfCraft** (com a funcionalidade de inversão de cores estilo SumatraPDF) no Windows em dois cenários:
1. **Ambiente Corporativo CAIXA** (com restrições de rede, proxy corporativo e políticas de segurança/AppLocker).
2. **Windows com Acesso de Administrador** (máquina pessoal ou sem restrições).

---

## 🏢 Cenário 1: Ambiente Corporativo CAIXA (Pastas e Rede Bloqueadas)

Em computadores da rede interna da CAIXA, existem três desafios principais resolvidos por este método:
1. **GPO / AppLocker (Erro 1260):** O Windows bloqueia a execução de binários `.exe` (incluindo scripts de build e compiladores) dentro de `C:\Users\` e `C:\TEMP\`. A pasta liberada para execução é `C:\Sistemas\<sua_matricula>\`.
2. **Bloqueio de Rede da CDN do Crates.io:** As conexões diretas para `crates.io` e `static.crates.io` são bloqueadas pelo firewall/proxy. A solução é usar o espelho público `rsproxy-sparse` e habilitar o Git oficial via Schannel.
3. **Dependência de C++ (aws-lc-sys):** Uma dependência criptográfica interna precisa do compilador C++ (`cl.exe`, `link.exe`) e dos cabeçalhos do Windows SDK (`windows.h`). As máquinas CAIXA com Visual Studio ou SDK possuem esses arquivos em `ScopeCppSDK`.

---

### Passo 1: Preparar as Pastas Liberadas

Abra o **PowerShell** e crie os diretórios na unidade permitida (substitua `c123710` pela sua matrícula):

```powershell
$matricula = $env:USERNAME   # ou sua matrícula, ex: c123710
New-Item -ItemType Directory -Force -Path "C:\Sistemas\$matricula\.cargo"
New-Item -ItemType Directory -Force -Path "C:\Sistemas\$matricula\.rustup"
New-Item -ItemType Directory -Force -Path "C:\Sistemas\$matricula\pdfcraft"
```

---

### Passo 2: Instalar o Rust nas Pastas Permitidas

1. Baixe o instalador oficial `rustup-init.exe` de [https://rustup.rs](https://rustup.rs).
2. Se o executável estiver em `Downloads`, copie-o para `C:\Sistemas\<matricula>\rustup-init.exe` para evitar o bloqueio de execução.
3. No PowerShell, defina as variáveis e execute a instalação:

```powershell
$matricula = $env:USERNAME

# Define as variáveis de ambiente permanentes do usuário
[Environment]::SetEnvironmentVariable("CARGO_HOME", "C:\Sistemas\$matricula\.cargo", "User")
[Environment]::SetEnvironmentVariable("RUSTUP_HOME", "C:\Sistemas\$matricula\.rustup", "User")

# Define para a sessão atual
$env:CARGO_HOME = "C:\Sistemas\$matricula\.cargo"
$env:RUSTUP_HOME = "C:\Sistemas\$matricula\.rustup"

# Executa o instalador
& "C:\Sistemas\$matricula\rustup-init.exe" -y --default-host x86_64-pc-windows-msvc
```

4. Adicione `C:\Sistemas\<matricula>\.cargo\bin` ao `PATH` do usuário no Windows.

---

### Passo 3: Configurar o Espelho de Rede e Pasta de Saída do Cargo

Crie ou edite o arquivo `C:\Sistemas\<sua_matricula>\.cargo\config.toml` com o seguinte conteúdo:

```toml
[build]
target-dir = "C:/Sistemas/<sua_matricula>/.cargo/target/pdfcraft"

[source.crates-io]
replace-with = 'rsproxy-sparse'

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[registries.rsproxy-sparse]
index = "sparse+https://rsproxy.cn/index/"

[net]
git-fetch-with-cli = true

[http]
check-revoke = false
```

> **Por que isso é necessário?**
> * `target-dir`: Faz com que os executáveis temporários de compilação sejam gerados e executados em `C:\Sistemas\...`, evitando o erro `GPO 1260`.
> * `rsproxy-sparse`: Permite baixar as bibliotecas Rust pela rede corporativa com resposta 100% liberada.
> * `git-fetch-with-cli = true`: Usa o Git oficial do Windows (que já possui os certificados corporativos e autenticação Schannel).

---

### Passo 4: Compilar Usando o Script Automatizado

Na raiz do repositório, existe o arquivo `compilar.bat`.

Basta **dar um duplo clique no `compilar.bat`** (ou executá-lo pelo terminal). 

O script configura automaticamente:
* As variáveis do Cargo (`CARGO_HOME`, `RUSTUP_HOME`, `CARGO_TARGET_DIR`);
* O compilador C++ e linker (`link.exe`, `cl.exe`);
* Os cabeçalhos do Windows SDK (`windows.h` em `SDK\include\um`, `ucrt`, `shared`);
* Compila o projeto e copia os executáveis limpos para a subpasta `C:\Sistemas\<sua_matricula>\pdfcraft\`:
  ```cmd
  cargo build --release -p pdfcraft -p pdfcraft-cli
  ```

---

### Passo 5: Executar o Aplicativo

Após a conclusão da compilação:
* Dê um duplo clique em `executar.bat` na raiz do projeto, ou
* Abra diretamente o executável na subpasta dedicada:
  ```text
  C:\Sistemas\<sua_matricula>\pdfcraft\pdfcraft.exe
  ```

---
---

## 💻 Cenário 2: Windows com Permissão de Administrador (Computador Pessoal / Casa)

Em computadores pessoais ou sem restrições de segurança/proxy, o processo é o padrão do ecossistema Rust.

### Passo 1: Instalar as Ferramentas de C++ do Visual Studio

O compilador Rust para Windows (`x86_64-pc-windows-msvc`) depende do linker e bibliotecas C++ da Microsoft:

1. Baixe o **Visual Studio Installer** ou **Build Tools for Visual Studio**:
   [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
2. Durante a instalação, selecione a carga de trabalho:
   * **Desenvolvimento para desktop com C++** *(Desktop development with C++)*.
3. Conclua a instalação.

---

### Passo 2: Instalar o Rust

1. Baixe o instalador oficial `rustup-init.exe` em [rustup.rs](https://rustup.rs).
2. Execute o instalador e aperte `Enter` para a instalação padrão (`1 - Proceed with standard installation`).
3. Ao finalizar, feche e abra novamente seu terminal (Prompt de Comando ou PowerShell) para carregar o `PATH`.
4. Verifique a instalação:
   ```cmd
   cargo --version
   rustc --version
   ```

---

### Passo 3: Clonar e Compilar o Projeto

1. Abra a pasta do projeto no terminal:
   ```cmd
   cd caminho\para\pdfcraft
   ```

2. Compile o aplicativo em modo Release otimizado:
   ```cmd
   cargo build --release -p pdfcraft
   ```

   *(Opcional: Para compilar também a ferramenta de linha de comando: `cargo build --release -p pdfcraft-cli`)*

---

### Passo 4: Executar o Aplicativo

O executável estará gerado em:
```text
target\release\pdfcraft.exe
```

Para executar:
```cmd
.\target\release\pdfcraft.exe
```

---

## 🎨 Como Usar a Inversão de Cores (Estilo SumatraPDF)

O PdfCraft conta com dois controles visuais distintos na barra superior (lado direito):

1. **☀️ / 🌙 Tema da Interface:**
   * Alterna entre o tema Claro e Escuro da moldura do programa (menus, abas e barras laterais).
   * O clique comum alterna direto entre **Claro e Escuro**.
   * Clicando com o botão direito, é possível escolher seguir o tema do Windows (*"Usar configuração do sistema"*).

2. **◐ Inversão de Cores do Documento (PDF):**
   * **Botão Contraste:** Localizado bem ao lado do botão de tema. Clicando nele, o PDF inverte as cores (fundo preto/escuro, textos e gráficos em branco/tons claros).
   * **Atalho de Teclado:** Pressione **`Ctrl + I`** ou **`Shift + I`** (ou a tecla rápida **`i`** quando não estiver editando texto).
   * **Menu Superior:** **Visualizar** ➔ **Inverter cores**.
   * **Persistência:** A preferência fica gravada. Ao abrir novos documentos ou reabrir o aplicativo, o modo de leitura invertido é mantido.
