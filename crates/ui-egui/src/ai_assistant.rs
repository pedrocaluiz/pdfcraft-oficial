//! Dedicated AI Assistant Panel (Ferramentas ▸ Assistente de IA).
//!
//! Provides interactive local AI assistance (Ollama / offline LLM) directly inside PdfCraft:
//! - Summarize (entire page or selected text)
//! - Ask questions with document/page context
//! - Translate (entire page or selected text to Portuguese)
//! - Insert results as sticky notes directly on the PDF
//! - Copy results to clipboard

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use egui::{Align, Align2, Color32, CornerRadius, Layout, Margin, Stroke, Ui, vec2};

use crate::lookup::{self, OllamaConfig};
use crate::theme::Tokens;
use crate::{PdfCraftApp, icons, theme, widgets};

/// Type of AI action requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiKind {
    Summarize,
    Ask,
    Translate,
}

impl AiKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Summarize => "Resumo",
            Self::Ask => "Pergunta",
            Self::Translate => "Tradução",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Summarize => "sparkles",
            Self::Ask => "message-square-text",
            Self::Translate => "languages",
        }
    }
}

/// A single AI interaction item (query and response).
#[derive(Clone, Debug)]
pub struct AiEntry {
    pub id: u64,
    pub kind: AiKind,
    pub title: String,
    pub query_preview: String,
    pub page: Option<usize>,
    pub response: String,
    pub loading: bool,
    pub error: Option<String>,
    pub copied: bool,
    pub note_added: bool,
}

/// State for the AI Assistant side panel.
pub struct AiPanelState {
    pub entries: Vec<AiEntry>,
    pub question_input: String,
    pub include_page_context: bool,
    tx: Sender<(u64, Result<String, String>)>,
    rx: Receiver<(u64, Result<String, String>)>,
    pub next_id: u64,
    pub focus_input: bool,
}

impl Default for AiPanelState {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self { entries: Vec::new(), question_input: String::new(), include_page_context: true, tx, rx, next_id: 1, focus_input: false }
    }
}

impl AiPanelState {
    /// Polls background AI generation threads and updates matching cards.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((id, res)) = self.rx.try_recv() {
            if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
                entry.loading = false;
                match res {
                    Ok(text) => {
                        entry.response = text;
                        entry.error = None;
                    }
                    Err(err) => {
                        entry.error = Some(err);
                    }
                }
                changed = true;
            }
        }
        changed
    }

    /// Submits a new background AI prompt.
    pub fn add_request(&mut self, kind: AiKind, title: String, query_preview: String, prompt: String, page: Option<usize>, config: &OllamaConfig) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        let entry =
            AiEntry { id, kind, title, query_preview, page, response: String::new(), loading: true, error: None, copied: false, note_added: false };

        // Put newest at the top
        self.entries.insert(0, entry);

        let tx = self.tx.clone();
        let cfg = config.clone();
        thread::spawn(move || {
            let res = lookup::query_ollama_raw(&prompt, None, &cfg);
            let _ = tx.send((id, res));
        });
    }
}

/// Renders the AI Assistant side panel.
pub fn panel(app: &mut PdfCraftApp, ui: &mut Ui, t: &Tokens) {
    // The header (Back / Close) is drawn by `panels::tool_detail`, which hosts this panel.
    let mut open_prefs = false;
    let mut delete_id = None;
    let mut insert_note_text = None;
    let mut trigger_action = None;
    let mut submit_question = false;

    // Status banner
    let is_enabled = app.ollama_config.enabled;
    let model_name = if app.ollama_config.model.trim().is_empty() { "llama3.2" } else { app.ollama_config.model.trim() };

    egui::Frame::NONE.fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            // Button first (right edge), then the status text takes whatever is left and truncates.
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 30.0), Layout::right_to_left(Align::Center), |ui| {
                if widgets::ghost_button(ui, "settings", "Configurar").on_hover_text("Abrir preferências do Ollama").clicked() {
                    open_prefs = true;
                }
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let (dot, text) = if is_enabled {
                        (Color32::from_rgb(34, 197, 94), egui::RichText::new(format!("Ollama: {model_name}")).strong().size(12.0))
                    } else {
                        (Color32::from_rgb(234, 179, 8), egui::RichText::new("Assistente Desativado").color(t.text_muted).size(12.0))
                    };
                    ui.label(egui::RichText::new("●").color(dot).size(12.0));
                    ui.add(egui::Label::new(text).truncate());
                });
            });
        },
    );

    ui.add_space(8.0);

    // Three Quick Action Buttons
    widgets::section_title(ui, "Ações Rápidas");
    ui.horizontal_wrapped(|ui| {
        let has_sel = app.active.and_then(|i| app.views.get(i)).and_then(|v| v.selected_text()).is_some();
        let sum_label = if has_sel { "Resumir Seleção" } else { "Resumir Página" };
        let trans_label = if has_sel { "Traduzir Seleção" } else { "Traduzir Página" };

        if widgets::icon_pill(ui, "sparkles", sum_label, false).on_hover_text("Gera um resumo estruturado com o modelo local").clicked() {
            trigger_action = Some(AiKind::Summarize);
        }
        if widgets::icon_pill(ui, "message-square-text", "Perguntar", false).on_hover_text("Digitar pergunta com contexto da página").clicked() {
            trigger_action = Some(AiKind::Ask);
        }
        if widgets::icon_pill(ui, "languages", trans_label, false).on_hover_text("Traduz o conteúdo para o português").clicked() {
            trigger_action = Some(AiKind::Translate);
        }
    });

    ui.add_space(10.0);

    // Results Feed & Question Bar layout
    let available_h = ui.available_height();
    let input_bar_h = 100.0;
    let list_h = (available_h - input_bar_h).max(120.0);

    // Results scroll area
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(list_h)
        .show(ui, |ui| {
            if app.ai_state.entries.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.add(icons::image("sparkles", 32.0, t.text_muted));
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Assistente de IA Local").strong().size(14.0));
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(
                            "Selecione um texto ou use os botões acima para resumir ou traduzir a página atual.\n\nVocê também pode fazer perguntas sobre o documento na caixa abaixo.",
                        )
                        .size(12.0)
                        .color(t.text_muted),
                    );
                });
            } else {
                for (idx, entry) in app.ai_state.entries.iter_mut().enumerate() {
                    let card_id = entry.id;
                    egui::Frame::NONE
                        .fill(t.card)
                        .stroke(Stroke::new(1.0, t.border))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());

                            // Card header
                            ui.horizontal(|ui| {
                                ui.add(icons::image(entry.kind.icon(), 16.0, t.icon));
                                // Reserve the badge and the remove button first; the title takes the rest.
                                let badge_w = if entry.page.is_some() { 56.0 } else { 0.0 };
                                let spacing = ui.spacing().item_spacing.x;
                                let title_w = (ui.available_width() - 26.0 - badge_w - spacing * 2.0).max(1.0);
                                ui.add_sized(
                                    [title_w, 20.0],
                                    egui::Label::new(egui::RichText::new(&entry.title).strong().size(13.0)).truncate(),
                                )
                                .on_hover_text(&entry.title);

                                if let Some(p) = entry.page {
                                    let (r, _) = ui.allocate_exact_size(vec2(52.0, 18.0), egui::Sense::hover());
                                    ui.painter().rect_filled(r, CornerRadius::same(4), t.pressed);
                                    ui.painter().text(r.center(), Align2::CENTER_CENTER, format!("Pág. {}", p + 1), theme::semibold(10.5), t.text_muted);
                                }

                                if icons::button(ui, "x", 26.0, false, "Remover").clicked() {
                                    delete_id = Some(card_id);
                                }
                            });

                            if !entry.query_preview.is_empty() {
                                ui.add_space(3.0);
                                ui.label(
                                    egui::RichText::new(format!("“{}”", entry.query_preview))
                                        .italics()
                                        .size(11.5)
                                        .color(t.text_faint),
                                );
                            }

                            ui.add_space(6.0);
                            ui.separator();
                            ui.add_space(6.0);

                            // Content state
                            if entry.loading {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label(egui::RichText::new(format!("Consultando Ollama ({model_name})...")).italics().color(t.text_muted));
                                });
                            } else if let Some(err) = &entry.error {
                                ui.label(egui::RichText::new(format!("Erro: {err}")).size(11.5).color(Color32::from_rgb(239, 68, 68)));
                                ui.add_space(4.0);
                                if widgets::ghost_button(ui, "settings", "Ajustar Configurações").clicked() {
                                    open_prefs = true;
                                }
                            } else {
                                ui.label(egui::RichText::new(&entry.response).size(12.5));

                                ui.add_space(8.0);
                                ui.separator();
                                ui.add_space(4.0);

                                // Card actions
                                ui.horizontal_wrapped(|ui| {
                                    let (copy_icon, copy_label) = if entry.copied { ("check", "Copiado") } else { ("copy", "Copiar") };
                                    if widgets::ghost_button(ui, copy_icon, copy_label).clicked() {
                                        ui.ctx().copy_text(entry.response.clone());
                                        entry.copied = true;
                                    }

                                    let (note_icon, note_label) =
                                        if entry.note_added { ("check", "Nota Adicionada") } else { ("pin", "Inserir como Nota no PDF") };
                                    if widgets::ghost_button(ui, note_icon, note_label).on_hover_text("Insere este resultado como uma anotação na página do PDF").clicked() {
                                        insert_note_text = Some(entry.response.clone());
                                        entry.note_added = true;
                                    }
                                });
                            }
                        });

                    if idx < 50 {
                        ui.add_space(6.0);
                    }
                }
            }
        });

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);

    // Bottom Question Input Bar
    egui::Frame::NONE.fill(t.card).stroke(Stroke::new(1.0, t.border)).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(8)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.add(widgets::checkbox(&mut app.ai_state.include_page_context, "Incluir texto da página atual"));

            ui.add_space(4.0);
            // Send button on the right, the field fills what is left of the row.
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 30.0), Layout::right_to_left(Align::Center), |ui| {
                let send_clicked = widgets::icon_pill(ui, "send", "Enviar", true).clicked();

                let text_edit = widgets::line(&mut app.ai_state.question_input)
                    .hint_text("Digite sua pergunta sobre o documento...")
                    .desired_width(ui.available_width());
                let resp = ui.add(text_edit);
                if app.ai_state.focus_input {
                    resp.request_focus();
                    app.ai_state.focus_input = false;
                }

                let enter_pressed = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if enter_pressed || send_clicked {
                    submit_question = true;
                }
            });
        },
    );

    // Handle deferred actions
    if let Some(id) = delete_id {
        app.ai_state.entries.retain(|e| e.id != id);
    }

    if let Some(text) = insert_note_text {
        app.insert_sticky_note(&text);
    }

    if open_prefs {
        app.dialog = Some(crate::Dialog::Preferences);
    }

    if let Some(kind) = trigger_action {
        app.trigger_ai_action(kind);
    }

    if submit_question {
        let q = app.ai_state.question_input.trim().to_string();
        if !q.is_empty() {
            let (page, context_text) = if app.ai_state.include_page_context {
                if let (Some(i), Some((_, id))) = (app.active, app.active_ids()) {
                    let p = app.views[i].current;
                    let txt = app.views[i].selected_text().or_else(|| {
                        app.views[i].texts.get(&p).map(|t| t.plain_text()).or_else(|| {
                            let doc = app.session.get(id)?;
                            let mut ex = pdfcraft_engine::export::Exporter::from_source(doc.export_source());
                            ex.text_of(&[p]).ok()
                        })
                    });
                    (Some(p), txt)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            let title = if q.chars().count() > 30 {
                let sub: String = q.chars().take(30).collect();
                format!("Pergunta: \"{sub}…\"")
            } else {
                format!("Pergunta: \"{q}\"")
            };

            let prompt = if let Some(ctx) = context_text {
                let p_num = page.map(|p| p + 1).unwrap_or(1);
                format!(
                    "Com base no texto da Página {p_num} do documento fornecido abaixo, responda à pergunta em português de forma clara, direta e objetiva:\n\nPergunta: {q}\n\nTexto de referência da Página {p_num}:\n\"\"\"\n{ctx}\n\"\"\""
                )
            } else {
                q.clone()
            };

            let preview = if q.chars().count() > 80 {
                let sub: String = q.chars().take(80).collect();
                format!("{sub}…")
            } else {
                q
            };

            app.ai_state.add_request(AiKind::Ask, title, preview, prompt, page, &app.ollama_config);
            app.ai_state.question_input.clear();
        }
    }
}
