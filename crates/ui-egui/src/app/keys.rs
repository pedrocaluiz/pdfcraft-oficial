use super::*;
use crate::page_keys::PageKeys;

impl PdfCraftApp {
    pub(super) fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::Key;
        // "Save changes?" is modal: its keys are its own (⌘D is Don't save there, not Document
        // properties; Escape cancels it rather than clearing a selection), and nothing may run
        // underneath it. They are read here, before the canvas can consume them.
        if self.close_request.is_some() {
            // These dialogs are drawn after the save prompt and own the keyboard while open.
            // Don't let Escape or Return resolve unsaved work underneath one.
            #[cfg(not(target_arch = "wasm32"))]
            if self.script_pending.as_ref().is_some_and(|pending| pending.has_prompt()) {
                return;
            }
            let covered = self.pending_link.is_some()
                || self.pending_remote_link.is_some()
                || self.pending_full_screen.is_some()
                || self.updates.open
                || self.dialog.is_some()
                || self.unlock_prompt.is_some();
            if !covered && let Some(choice) = dialogs::save_prompt_key(ctx, &mut self.save_prompt_button) {
                self.resolve_close(ctx, choice);
            }
            return;
        }
        // The other dialogs and prompts are modal too (⌘W closed the file behind Preferences,
        // #870).
        if self.modal_open() {
            return;
        }
        if let Some(view) = self.active.and_then(|i| self.views.get_mut(i))
            && view.auto_scroll.escape(ctx)
        {
            return;
        }
        // The in-window menu bar holds the keys while one of its menus is open or it has the
        // keyboard (Alt, F10); a command shortcut has left it already (`logic`) and runs. The
        // chords wait meanwhile: the registered commands', Select all and the view's.
        let chords = !craft_menu::bar::is_open(ctx) && !craft_menu::bar::is_focused(ctx);
        // The pages' own chords (Select all, the view's) don't reach pages something covers: the
        // full-screen and update prompts, or the OCR Verify screen in their place (a dialog has
        // returned above, and the palette holds the keyboard).
        let page_chords = !self.covers_pages();
        if chords {
            let selection_shortcut_handled = self.selection_lookup_keys(ctx);
            if !selection_shortcut_handled {
                self.registry_shortcuts(ctx);
            }
        }
        // (Esc closes a Combine file shown large first.)
        if self.full_screen && self.combine_tab.preview.is_none() && self.page_keys.pressed(ctx, Key::Escape) {
            self.set_full_screen(ctx, false);
        }
        if let Some(i) = self.active {
            if page_chords && chords {
                self.select_all_keys(i, ctx);
            }
            self.tool_keys(i, ctx);
            let keys = self.page_keys;
            if self.quick_tool == QuickTool::EditText
                && let Some(doc) = self.session.get(self.views[i].id).filter(|d| d.allows_modification())
            {
                object_ui::nudge_keys(ctx, keys, &mut self.views[i], doc);
                object_ui::delete_keys(ctx, keys, &mut self.views[i]);
            } else if let Some(doc) = self.session.get(self.views[i].id).filter(|d| d.allows_annotation()) {
                comments::nudge_keys(ctx, keys, &mut self.views[i], &doc.info);
            }
            // A selected field moves with the arrows too (Prepare a form, #2608), before the view
            // turns the page with them.
            if self.is_preparing()
                && let Some(doc) = self.session.get(self.views[i].id).filter(|d| d.allows_modification())
            {
                prepare::nudge_keys(ctx, keys, &mut self.views[i], doc);
            }
            canvas::shortcuts(&mut self.views[i], ctx, keys, chords && page_chords, self.scroll_speed);
            self.clipboard_keys(i, ctx);
            self.image_pick_keys(i, ctx);
        }
    }
}

impl PdfCraftApp {
    /// ⌘T, ⌘D, ⌘E and ⇧⌘E on the selected text: Translate, Dictionary, and the AI assistant's
    /// Explain and Summarize. Whether one ran, so the registered commands wait.
    fn selection_lookup_keys(&mut self, ctx: &egui::Context) -> bool {
        use egui::Key;
        let mut selection_shortcut_handled = false;
        if let Some(i) = self.active
            && self.dialog.is_none()
            && !self.palette_open
            && !ctx.egui_wants_keyboard_input()
            && let Some((text, rect)) = self.views.get(i).and_then(|v| v.selected_text_and_rect())
        {
            let pos = egui::pos2(rect.left(), rect.bottom() + 6.0);
            if ctx.input_mut(|input| input.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, Key::T))) {
                self.active_lookup =
                    Some(crate::lookup::ActiveLookup::start(&text, crate::lookup::LookupKind::Translate, pos, None, Some(&self.ollama_config)));
                selection_shortcut_handled = true;
            } else if ctx.input_mut(|input| input.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, Key::D))) {
                self.active_lookup =
                    Some(crate::lookup::ActiveLookup::start(&text, crate::lookup::LookupKind::Dictionary, pos, None, Some(&self.ollama_config)));
                selection_shortcut_handled = true;
            } else if self.ollama_config.enabled
                && ctx.input_mut(|input| input.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, Key::E)))
            {
                self.active_lookup =
                    Some(crate::lookup::ActiveLookup::start(&text, crate::lookup::LookupKind::AiExplain, pos, None, Some(&self.ollama_config)));
                selection_shortcut_handled = true;
            } else if self.ollama_config.enabled
                && ctx.input_mut(|input| {
                    input.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, Key::E))
                })
            {
                self.active_lookup =
                    Some(crate::lookup::ActiveLookup::start(&text, crate::lookup::LookupKind::AiSummarize, pos, None, Some(&self.ollama_config)));
                selection_shortcut_handled = true;
            }
        }
        selection_shortcut_handled
    }
}

impl PdfCraftApp {
    /// ⌘A selects all of the document or the page grid, unless a text field or overlay owns the
    /// keyboard.
    fn select_all_keys(&mut self, i: usize, ctx: &egui::Context) {
        use egui::Key;
        let pages_owns_keyboard =
            self.right == Some(RightPanel::Pages) && self.views[i].pages_focus_id.is_some_and(|id| ctx.memory(|m| m.has_focus(id)));
        if self.dialog.is_none()
            && !self.palette_open
            && (pages_owns_keyboard || !ctx.egui_wants_keyboard_input())
            && ctx.input_mut(|input| input.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, Key::A)))
        {
            let view = &self.views[i];
            // A paragraph or text box open for typing keeps ⌘A for its own text.
            let edit_objects = self.quick_tool == QuickTool::EditText && view.line_editor.is_none() && view.content.draft.is_none();
            if pages_owns_keyboard {
                self.views[i].select_all_pages();
            } else if let Some(doc) = self.session.get(view.id).filter(|d| edit_objects && d.allows_modification()) {
                object_ui::select_all(&mut self.views[i], doc);
            } else if self.is_preparing() {
                if let Some(doc) = self.session.get(self.views[i].id) {
                    prepare::select_all(&mut self.views[i], &doc.form);
                }
            } else {
                self.views[i].select_all();
            }
        }
    }

    /// The quick tools' keys, as in Acrobat: V selects, H pans, and holding Space pans until it
    /// is released. Plain letters, so not while a text field, a form field, a dialog, the palette
    /// or a menu has the keyboard.
    fn tool_keys(&mut self, i: usize, ctx: &egui::Context) {
        use egui::Key;
        let keys = self.page_keys;
        let free = self.dialog.is_none() && !self.palette_open && !ctx.egui_wants_keyboard_input() && self.views[i].forms.focus.is_none();
        let free = free && !keys.over();
        let (plain, space) = (ctx.input(|input| input.modifiers.is_none()), PageKeys::held(ctx, Key::Space));
        if let Some(previous) = self.space_hand
            && !space
        {
            self.space_hand = None;
            if self.quick_tool == QuickTool::Hand {
                self.quick_tool = previous;
            }
        }
        if !free || !plain {
            return;
        }
        if keys.pressed(ctx, Key::V) {
            self.quick_tool = QuickTool::Select;
            self.space_hand = None;
        }
        if keys.pressed(ctx, Key::H) {
            self.quick_tool = QuickTool::Hand;
            self.space_hand = None;
        }
        if keys.pressed(ctx, Key::A) {
            let doc_permits = self.session.get(self.views[i].id).is_some_and(|d| d.allows_annotation());
            let selection = if doc_permits { self.session.get(self.views[i].id).and_then(|d| self.views[i].selection_quads(&d.info)) } else { None };
            if let Some((page, quads)) = selection {
                self.views[i].clear_selection();
                let tool = comments::CommentTool::Highlight;
                self.comment_prefs.group_tool[tool.group()] = tool;
                let style = self.comment_prefs.style(tool);
                let author = self.comment_prefs.author.clone();
                let shape = pdfcraft_engine::Shape::TextMarkup { kind: pdfcraft_engine::Markup::Highlight, quads };
                self.apply_edit(pdfcraft_engine::Edit::AddAnnotation(pdfcraft_engine::NewAnnotation {
                    page,
                    shape,
                    style,
                    contents: String::new(),
                    author,
                }));
            } else if self.quick_tool == QuickTool::Comment(comments::CommentTool::Highlight) {
                self.quick_tool = QuickTool::Select;
            } else if doc_permits {
                let tool = comments::CommentTool::Highlight;
                self.comment_prefs.group_tool[tool.group()] = tool;
                self.quick_tool = QuickTool::Comment(tool);
            }
            self.space_hand = None;
        }
        if space && self.space_hand.is_none() && self.quick_tool != QuickTool::Hand {
            self.space_hand = Some(self.quick_tool);
            self.quick_tool = QuickTool::Hand;
        }
    }
}
