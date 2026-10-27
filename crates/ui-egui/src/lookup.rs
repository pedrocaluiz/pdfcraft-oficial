//! Smart In-App Selection Handlers (Translation & Dictionary lookups).
//!
//! Provides inline in-app definitions (Dicio) and translations (Google Translate),
//! as well as configurable external SelectionHandlers (SumatraPDF parity).

use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Duration;

use egui::{Color32, CornerRadius, Id, Margin, Order, Pos2, Rect, Stroke, Ui};
use serde::{Deserialize, Serialize};

use crate::PdfCraftApp;

/// Configurable selection handler (matches SumatraPDF SelectionHandlers).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionHandler {
    pub name: String,
    pub url: String,
    pub shortcut: String,
}

impl SelectionHandler {
    /// Default selection handlers.
    pub fn defaults() -> Vec<Self> {
        vec![
            Self {
                name: "Google Tradutor PT-BR".into(),
                url: "https://translate.google.com/?sl=auto&tl=pt&text=${selection}".into(),
                shortcut: "Ctrl+T".into(),
            },
            Self {
                name: "Definição (Dicio)".into(), url: "https://www.dicio.com.br/pesquisa.php?q=${selection}".into(), shortcut: "Ctrl+D".into()
            },
            Self { name: "Traduzir com DeepL".into(), url: "https://www.deepl.com/translator#auto/pt/${selection}".into(), shortcut: String::new() },
            Self { name: "Procurar com Google".into(), url: "https://www.google.com/search?q=${selection}".into(), shortcut: String::new() },
            Self { name: "Procurar com Bing".into(), url: "https://www.bing.com/search?q=${selection}".into(), shortcut: String::new() },
            Self {
                name: "Procurar na Wikipedia".into(),
                url: "https://pt.wikipedia.org/wiki/Special:Search?search=${selection}".into(),
                shortcut: String::new(),
            },
            Self { name: "Google Scholar".into(), url: "https://scholar.google.com/scholar?q=${selection}".into(), shortcut: String::new() },
        ]
    }

    /// Formats the target URL by replacing `${selection}` with URL-encoded query.
    pub fn format_url(&self, selection: &str) -> String {
        let encoded = url_encode(selection);
        self.url.replace("$${selection}", &encoded).replace("${selection}", &encoded)
    }
}

/// URL encode string (percent-encoding).
pub fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 2);
    for b in input.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

/// Configuration for local Ollama AI assistant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub model: String,
    pub system_prompt: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "http://localhost:11434".into(),
            model: "llama3.2".into(),
            system_prompt:
                "Você é um assistente útil e conciso integrado a um leitor de PDF. Responda em português de forma clara, didática e direta.".into(),
        }
    }
}

/// The kind of lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookupKind {
    Translate,
    Dictionary,
    AiExplain,
    AiSummarize,
}

/// Current status of the in-app lookup.
#[derive(Clone, Debug)]
pub enum LookupStatus {
    Loading,
    Ready(LookupResult),
    Error(String),
}

/// Result returned from background query.
#[derive(Clone, Debug)]
pub struct LookupResult {
    pub title: String,
    pub text: String,
    pub source_lang: Option<String>,
    pub definitions: Vec<String>,
}

/// Currently active in-app lookup popup state.
pub struct ActiveLookup {
    pub query: String,
    pub kind: LookupKind,
    pub status: LookupStatus,
    pub target_url: String,
    pub screen_pos: Pos2,
    pub copied: bool,
    pub note_added: bool,
    pub frames_alive: u32,
    receiver: Option<Receiver<Result<LookupResult, String>>>,
}

impl ActiveLookup {
    /// Starts a background lookup.
    pub fn start(query: &str, kind: LookupKind, screen_pos: Pos2, fallback_url: Option<&str>, ollama_config: Option<&OllamaConfig>) -> Self {
        let trimmed = query.trim().to_string();
        let target_url = match fallback_url {
            Some(u) => {
                let encoded = url_encode(&trimmed);
                u.replace("$${selection}", &encoded).replace("${selection}", &encoded)
            }
            None => match kind {
                LookupKind::Translate => format!("https://translate.google.com/?sl=auto&tl=pt&text={}", url_encode(&trimmed)),
                LookupKind::Dictionary => format!("https://www.dicio.com.br/pesquisa.php?q={}", url_encode(&trimmed)),
                LookupKind::AiExplain | LookupKind::AiSummarize => {
                    ollama_config.map(|c| c.endpoint.clone()).unwrap_or_else(|| "http://localhost:11434".into())
                }
            },
        };

        let (sender, receiver) = channel();
        let q = trimmed.clone();
        let cfg = ollama_config.cloned();
        thread::spawn(move || {
            let res = perform_fetch(kind, &q, cfg.as_ref());
            let _ = sender.send(res);
        });

        Self {
            query: trimmed,
            kind,
            status: LookupStatus::Loading,
            target_url,
            screen_pos,
            copied: false,
            note_added: false,
            frames_alive: 0,
            receiver: Some(receiver),
        }
    }

    /// Checks if the background query has finished.
    pub fn poll(&mut self) -> bool {
        if let Some(rx) = &self.receiver {
            match rx.try_recv() {
                Ok(Ok(res)) => {
                    self.status = LookupStatus::Ready(res);
                    self.receiver = None;
                    return true;
                }
                Ok(Err(err)) => {
                    self.status = LookupStatus::Error(err);
                    self.receiver = None;
                    return true;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if matches!(self.status, LookupStatus::Loading) {
                        self.status = LookupStatus::Error("Consulta cancelada".into());
                    }
                    self.receiver = None;
                    return true;
                }
            }
        }
        false
    }
}

/// Performs the background HTTP query.
fn perform_fetch(kind: LookupKind, query: &str, ollama: Option<&OllamaConfig>) -> Result<LookupResult, String> {
    match kind {
        LookupKind::Translate => fetch_google_translate(query),
        LookupKind::Dictionary => {
            // For single words, attempt Dicio Portuguese dictionary first
            if !query.contains(' ')
                && let Ok(res) = fetch_dicio(query)
            {
                return Ok(res);
            }
            // Fallback to Google definitions & translation
            fetch_google_translate(query)
        }
        LookupKind::AiExplain => fetch_ollama(query, "Explique o seguinte trecho de forma didática, clara e concisa:", ollama),
        LookupKind::AiSummarize => fetch_ollama(query, "Faça um resumo objetivo com os pontos centrais do seguinte trecho:", ollama),
    }
}

/// Queries local Ollama instance with a prompt and returns the raw response text.
pub fn query_ollama_raw(prompt: &str, system: Option<&str>, config: &OllamaConfig) -> Result<String, String> {
    let endpoint = config.endpoint.trim().trim_end_matches('/');
    let model = if config.model.trim().is_empty() { "llama3.2" } else { config.model.trim() };
    let url = format!("{endpoint}/api/generate");

    let sys = system.unwrap_or(&config.system_prompt);
    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "system": sys,
        "stream": false
    });

    #[cfg(not(target_arch = "wasm32"))]
    {
        let agent = build_agent(Duration::from_secs(300))?;
        let body_str = serde_json::to_string(&body).map_err(|e| e.to_string())?;
        let mut resp = agent
            .post(&url)
            .header("Content-Type", "application/json")
            .send(body_str.as_bytes())
            .map_err(|e| {
                format!(
                    "Não foi possível conectar ao Ollama em {}.\n\nDetalhes: {}\n\nCertifique-se de que o Ollama está rodando no computador (ex.: 'ollama run {}').",
                    endpoint, e, model
                )
            })?;

        let body_resp = resp.body_mut().with_config().limit(1 << 23).read_to_string().map_err(|e| e.to_string())?;
        let val: serde_json::Value = serde_json::from_str(&body_resp).map_err(|e| format!("Erro ao decodificar JSON do Ollama: {e}"))?;
        let response_text = val.get("response").and_then(|v| v.as_str()).unwrap_or("Sem resposta retornada pelo modelo.").trim();

        Ok(response_text.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err("Ollama local não é suportado no navegador".into())
    }
}

/// Queries local Ollama instance for text generation/explanation.
fn fetch_ollama(query: &str, instruction: &str, config: Option<&OllamaConfig>) -> Result<LookupResult, String> {
    let default_cfg = OllamaConfig::default();
    let cfg = config.unwrap_or(&default_cfg);
    let model = if cfg.model.trim().is_empty() { "llama3.2" } else { cfg.model.trim() };
    let full_prompt = format!("{instruction}\n\n\"\"\"\n{query}\n\"\"\"");
    let response_text = query_ollama_raw(&full_prompt, None, cfg)?;

    Ok(LookupResult { title: format!("Assistente IA ({model})"), text: response_text, source_lang: None, definitions: Vec::new() })
}

/// Tests connection to Ollama and returns available installed models.
pub fn test_ollama_connection(endpoint: &str) -> Result<Vec<String>, String> {
    let endpoint = endpoint.trim().trim_end_matches('/');
    let url = format!("{endpoint}/api/tags");
    #[cfg(not(target_arch = "wasm32"))]
    {
        let agent = build_agent(Duration::from_secs(10))?;
        let mut resp = agent.get(&url).call().map_err(|e| format!("Falha de conexão em {}: {}", endpoint, e))?;
        let body = resp.body_mut().with_config().limit(1 << 20).read_to_string().map_err(|e| e.to_string())?;
        let val: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("Erro ao decodificar JSON: {e}"))?;
        let mut models = Vec::new();
        if let Some(list) = val.get("models").and_then(|m| m.as_array()) {
            for m in list {
                if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                    models.push(name.to_string());
                }
            }
        }
        Ok(models)
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err("Não suportado em wasm32".into())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn build_agent(timeout: Duration) -> Result<ureq::Agent, String> {
    let native = rustls_native_certs::load_native_certs();
    let certs: Vec<ureq::tls::Certificate<'static>> =
        native.certs.into_iter().map(|c| ureq::tls::Certificate::from_der(c.as_ref()).to_owned()).collect();
    let tls = if certs.is_empty() {
        ureq::tls::TlsConfig::builder().build()
    } else {
        ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::new_with_certs(&certs)).build()
    };

    Ok(ureq::Agent::config_builder().timeout_global(Some(timeout)).tls_config(tls).build().new_agent())
}

#[cfg(target_arch = "wasm32")]
pub fn build_agent(_timeout: Duration) -> Result<(), String> {
    Err("Networking not supported on wasm".into())
}

/// Fetches Google Translate response.
fn fetch_google_translate(query: &str) -> Result<LookupResult, String> {
    let url = format!("https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl=pt&dt=t&dt=bd&dt=md&q={}", url_encode(query));

    let agent = build_agent(Duration::from_secs(10))?;
    let mut resp =
        agent.get(&url).header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)").call().map_err(|e| format!("Falha de conexão: {e}"))?;

    let body = resp.body_mut().with_config().limit(1 << 20).read_to_string().map_err(|e| e.to_string())?;
    let val: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;

    let mut translated = String::new();
    if let Some(arr) = val.get(0).and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(part) = item.get(0).and_then(|v| v.as_str()) {
                translated.push_str(part);
            }
        }
    }

    if translated.is_empty() {
        return Err("Nenhuma tradução encontrada".into());
    }

    let source_lang = val.get(2).and_then(|v| v.as_str()).map(|s| s.to_string());

    let mut definitions = Vec::new();
    // Index 12 contains definitions
    if let Some(defs_arr) = val.get(12).and_then(|v| v.as_array()) {
        for group in defs_arr {
            let pos = group.get(0).and_then(|v| v.as_str()).unwrap_or("");
            if let Some(entries) = group.get(1).and_then(|v| v.as_array()) {
                for entry in entries.iter().take(3) {
                    if let Some(d) = entry.get(0).and_then(|v| v.as_str()) {
                        if !pos.is_empty() {
                            definitions.push(format!("[{pos}] {d}"));
                        } else {
                            definitions.push(d.to_string());
                        }
                    }
                }
            }
        }
    }
    // Index 1 contains dictionary alternatives / synonyms
    if definitions.is_empty()
        && let Some(dict_arr) = val.get(1).and_then(|v| v.as_array())
    {
        for group in dict_arr {
            let pos = group.get(0).and_then(|v| v.as_str()).unwrap_or("");
            if let Some(terms) = group.get(1).and_then(|v| v.as_array()) {
                let words: Vec<&str> = terms.iter().filter_map(|t| t.as_str()).take(4).collect();
                if !words.is_empty() {
                    definitions.push(format!("[{pos}] {}", words.join(", ")));
                }
            }
        }
    }

    Ok(LookupResult { title: "Google Tradutor (PT-BR)".into(), text: translated, source_lang, definitions })
}

/// Fetches Dicio definition (Portuguese dictionary).
fn fetch_dicio(word: &str) -> Result<LookupResult, String> {
    let url = format!("https://www.dicio.com.br/pesquisa.php?q={}", url_encode(word));
    let agent = build_agent(Duration::from_secs(10))?;
    let mut resp =
        agent.get(&url).header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)").call().map_err(|e| format!("Falha de conexão: {e}"))?;

    let html = resp.body_mut().with_config().limit(1 << 20).read_to_string().map_err(|e| e.to_string())?;

    // Look for <p class="significado textonovo"> or <p class="significado">
    let Some(start_tag) = html.find("class=\"significado") else {
        return Err("Definição não encontrada no Dicio".into());
    };
    let Some(tag_end) = html[start_tag..].find('>') else {
        return Err("Formato inesperado".into());
    };
    let content_start = start_tag + tag_end + 1;
    let Some(content_end) = html[content_start..].find("</p>") else {
        return Err("Formato inesperado".into());
    };

    let raw_snippet = &html[content_start..content_start + content_end];
    let cleaned = strip_html_tags(raw_snippet);
    if cleaned.trim().is_empty() {
        return Err("Definição vazia".into());
    }

    Ok(LookupResult { title: "Definição (Dicio)".into(), text: cleaned, source_lang: Some("pt".into()), definitions: Vec::new() })
}

/// Strips HTML tags and normalizes whitespace.
fn strip_html_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut inside = false;
    for ch in input.chars() {
        if ch == '<' {
            inside = true;
            result.push(' ');
        } else if ch == '>' {
            inside = false;
        } else if !inside {
            result.push(ch);
        }
    }
    // Normalize spaces
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Translates language code to display name.
fn lang_name(code: &str) -> &str {
    match code {
        "en" => "Inglês",
        "es" => "Espanhol",
        "fr" => "Francês",
        "de" => "Alemão",
        "it" => "Italiano",
        "pt" => "Português",
        "ja" => "Japonês",
        "zh" | "zh-CN" => "Chinês",
        "ru" => "Russo",
        _ => code,
    }
}

/// Renders the in-app lookup balloon on the screen.
pub fn render_balloon(ui: &mut Ui, app: &mut PdfCraftApp, viewport: Rect) {
    let mut poll_repaint = false;
    if let Some(active) = app.active_lookup.as_mut()
        && active.poll()
    {
        poll_repaint = true;
    }
    if poll_repaint {
        ui.ctx().request_repaint();
    }

    if let Some(active) = app.active_lookup.as_mut() {
        active.frames_alive = active.frames_alive.saturating_add(1);
    }

    let Some(active) = &app.active_lookup else { return };

    // Closes on Escape
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.active_lookup = None;
        return;
    }

    let balloon_rect_id = Id::unique("selection-lookup-balloon-rect");
    let previous_rect = ui.ctx().data(|d| d.get_temp::<Rect>(balloon_rect_id));

    // Closes on clicking outside the balloon (left or right click)
    let clicked_outside = ui.input(|i| {
        (i.pointer.button_pressed(egui::PointerButton::Primary) || i.pointer.button_pressed(egui::PointerButton::Secondary))
            && i.pointer.interact_pos().is_some_and(|pos| previous_rect.is_some_and(|rect| !rect.contains(pos)))
    });

    if active.frames_alive > 1 && clicked_outside {
        app.active_lookup = None;
        return;
    }

    let is_ai = matches!(active.kind, LookupKind::AiExplain | LookupKind::AiSummarize);
    let balloon_width = if is_ai { 460.0f32 } else { 360.0f32 };
    // Position below or above the selected text
    let x = (active.screen_pos.x).clamp(viewport.left() + 10.0, (viewport.right() - balloon_width - 10.0).max(viewport.left() + 10.0));
    let y = if active.screen_pos.y + 260.0 < viewport.bottom() {
        active.screen_pos.y + 12.0
    } else {
        (active.screen_pos.y - 250.0).max(viewport.top() + 10.0)
    };

    let mut close = false;
    let mut copy_text = None;
    let mut insert_note = None;
    let mut open_browser = None;
    let mut open_prefs = false;

    let id = Id::unique("selection-lookup-card");
    let area = egui::Area::new(id).order(Order::Foreground).fixed_pos(Pos2::new(x, y)).show(ui.ctx(), |ui| {
        let t = crate::theme::Tokens::get(ui.ctx());

        egui::Frame::NONE
            .fill(t.card)
            .stroke(Stroke::new(1.0, t.border))
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(14))
            .shadow(egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(if t.dark() { 120 } else { 40 }) })
            .show(ui, |ui| {
                ui.set_width(balloon_width);

                // Header
                ui.horizontal(|ui| {
                    let header_title = match &active.status {
                        LookupStatus::Ready(res) => res.title.as_str(),
                        _ => match active.kind {
                            LookupKind::Translate => "Google Tradutor",
                            LookupKind::Dictionary => "Dicionário",
                            LookupKind::AiExplain => "Assistente IA: Explicação",
                            LookupKind::AiSummarize => "Assistente IA: Resumo",
                        },
                    };

                    let icon = match active.kind {
                        LookupKind::Translate => "🌐",
                        LookupKind::Dictionary => "📖",
                        LookupKind::AiExplain => "🤖",
                        LookupKind::AiSummarize => "📝",
                    };

                    ui.label(egui::RichText::new(format!("{icon} {header_title}")).strong().size(13.0));

                    if let LookupStatus::Ready(res) = &active.status
                        && let Some(lang) = &res.source_lang
                    {
                        let badge = format!("{} ➔ PT", lang_name(lang));
                        ui.label(egui::RichText::new(badge).size(11.0).color(Color32::from_rgb(24, 128, 56)));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("✕").on_hover_text("Fechar (Esc)").clicked() {
                            close = true;
                        }
                        if is_ai {
                            if ui.button("⚙").on_hover_text("Configurações do Ollama").clicked() {
                                open_prefs = true;
                            }
                        } else if ui.button("🌐").on_hover_text("Abrir no navegador").clicked() {
                            open_browser = Some(active.target_url.clone());
                        }
                        if let LookupStatus::Ready(res) = &active.status {
                            let btn_label = if active.copied { "✓ Copiado" } else { "📋 Copiar" };
                            if ui.button(btn_label).clicked() {
                                copy_text = Some(res.text.clone());
                            }
                            let note_label = if active.note_added { "✓ Nota Inserida" } else { "📌 Inserir como Nota" };
                            if ui.button(note_label).on_hover_text("Inserir este resultado como nota/anotação no PDF").clicked() {
                                insert_note = Some(res.text.clone());
                            }
                        }
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                // Content
                match &active.status {
                    LookupStatus::Loading => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            let msg = if is_ai { "Consultando Ollama..." } else { "Consultando..." };
                            ui.label(egui::RichText::new(msg).italics());
                        });
                    }
                    LookupStatus::Ready(res) => {
                        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&res.text).size(13.5));

                                if !res.definitions.is_empty() {
                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new("Definições / Significados:").size(12.0).strong());
                                    for def in &res.definitions {
                                        ui.horizontal(|ui| {
                                            ui.label("•");
                                            ui.label(egui::RichText::new(def).size(12.0));
                                        });
                                    }
                                }
                            });
                        });
                    }
                    LookupStatus::Error(err) => {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Não foi possível carregar a resposta.").color(Color32::from_rgb(217, 48, 37)));
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(err).size(11.0));
                            ui.add_space(8.0);
                            if is_ai {
                                if ui.button("⚙ Configurar Ollama em Preferências").clicked() {
                                    open_prefs = true;
                                }
                            } else if ui.button("🌐 Abrir no Navegador Web").clicked() {
                                open_browser = Some(active.target_url.clone());
                            }
                        });
                    }
                }
            });
    });

    ui.ctx().data_mut(|d| d.insert_temp(balloon_rect_id, area.response.rect));

    if active.frames_alive > 1 && area.response.clicked_elsewhere() {
        close = true;
    }

    if let Some(url) = open_browser {
        app.open_url(&url);
        app.active_lookup = None;
    }

    if let Some(text) = copy_text {
        ui.ctx().copy_text(text);
        if let Some(active) = app.active_lookup.as_mut() {
            active.copied = true;
        }
    }

    if let Some(text) = insert_note {
        app.insert_sticky_note(&text);
        if let Some(active) = app.active_lookup.as_mut() {
            active.note_added = true;
        }
    }

    if open_prefs {
        app.dialog = Some(crate::Dialog::Preferences);
        app.active_lookup = None;
    }

    if close {
        app.active_lookup = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("maçã"), "ma%C3%A7%C3%A3");
        assert_eq!(url_encode("a & b = c"), "a+%26+b+%3D+c");
    }

    #[test]
    fn test_format_url() {
        let handler =
            SelectionHandler { name: "Dicio".into(), url: "https://www.dicio.com.br/pesquisa.php?q=${selection}".into(), shortcut: "Ctrl+D".into() };
        assert_eq!(handler.format_url("palavra chave"), "https://www.dicio.com.br/pesquisa.php?q=palavra+chave");

        let sumatra_style = SelectionHandler { name: "Sumatra".into(), url: "https://example.com/?q=$${selection}".into(), shortcut: "".into() };
        assert_eq!(sumatra_style.format_url("test"), "https://example.com/?q=test");
    }

    #[test]
    fn test_strip_html_tags() {
        let html = "<p class=\"significado\">Significado de <b>exemplo</b>: <span>modelo</span>.</p>";
        assert_eq!(strip_html_tags(html), "Significado de exemplo : modelo .");
    }

    #[test]
    fn test_defaults() {
        let defs = SelectionHandler::defaults();
        assert!(defs.iter().any(|h| h.name.contains("Google Tradutor")));
        assert!(defs.iter().any(|h| h.name.contains("Dicio")));
    }

    #[test]
    fn test_serialization() {
        let defs = SelectionHandler::defaults();
        let json = serde_json::to_string(&defs).unwrap_or_default();
        assert!(!json.is_empty());
        let back: Vec<SelectionHandler> = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(defs, back);
    }
}
