//! Slash commands in the composer: finding them as `/` is typed, the form
//! for a command's options, and running it.

use gpui_kit::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _,
    Window, div, px, rgb,
    component::input::{Input, InputState},
};
use oxicord_model::{CommandOption, SlashCommand, command::kind};

use super::ChatView;
use crate::{
    account::Load,
    backend::Command,
    chat::{
        autocomplete::{self, Suggestion},
        command_form::{self, Entry},
    },
    icons::icon,
    search_query, theme,
};

/// One option's input in the form.
pub(super) struct FormField {
    option: CommandOption,
    entry: Entry,
    /// Text options have a box, boolean and choice ones do not.
    input: Option<Entity<InputState>>,
}

pub(super) struct CommandForm {
    command: SlashCommand,
    fields: Vec<FormField>,
    /// Why the last attempt to run it failed.
    error: Option<String>,
}

impl ChatView {
    /// Suggestions for a `/` at the start of the text, asking the server for
    /// the channel's commands the first time.
    pub(super) fn command_suggestions(&mut self, query: &str, cx: &mut Context<Self>) -> Vec<Suggestion> {
        let Some(channel) = self.channel else { return Vec::new() };
        let backend = self.backend.clone();
        self.session.update(cx, |session, _| session.ensure_commands(&backend, channel));

        match self.session.read(cx).commands.get(&channel) {
            Some(Load::Ready(list)) => autocomplete::command_suggestions(&list.commands, &list.apps, query),
            _ => Vec::new(),
        }
    }

    /// Opens the option form for the picked suggestion.
    pub(super) fn open_command_form(&mut self, pick: &Suggestion, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(channel), Some(index)) = (self.channel, pick.command) else { return };
        // The pick names an entry of the list as it was when the suggestion
        // was built, so match on the label rather than trusting the index to
        // survive a reload.
        let command = match self.session.read(cx).commands.get(&channel) {
            Some(Load::Ready(list)) => list
                .commands
                .get(index)
                .filter(|c| format!("/{}", c.name) == pick.label)
                .cloned(),
            _ => None,
        };
        let Some(command) = command else { return };

        let fields = command
            .options
            .iter()
            .map(|option| FormField {
                option: option.clone(),
                entry: command_form::default_entry(option),
                input: (option.choices.is_empty() && option.kind != kind::BOOLEAN).then(|| {
                    let hint = option.description.clone();
                    cx.new(|cx| InputState::new(window, cx).placeholder(hint))
                }),
            })
            .collect();
        self.command_form = Some(CommandForm { command, fields, error: None });
        self.input.update(cx, |input, cx| input.set_value("", window, cx));
        self.suggestions.clear();
        cx.notify();
    }

    /// Reads the form and sends the command.
    fn run_command(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &self.command_form else { return };
        let (Some(channel), Some(session_id)) = (
            self.channel,
            self.session.read(cx).store.session_id().map(str::to_owned),
        ) else {
            return;
        };

        let entries: Vec<Entry> = form
            .fields
            .iter()
            .map(|f| match (&f.input, &f.entry) {
                (Some(input), Entry::Text(_)) => Entry::Text(input.read(cx).value().to_string()),
                (_, other) => other.clone(),
            })
            .collect();
        let options: Vec<CommandOption> = form.fields.iter().map(|f| f.option.clone()).collect();
        let guild = self.guild(cx);

        let result = {
            let store = &self.session.read(cx).store;
            let resolve = |kind: u8, text: &str| -> Option<String> {
                match kind {
                    kind::USER => search_query::resolve_user(store, guild, channel, text).map(|i| i.to_string()),
                    kind::CHANNEL => search_query::resolve_channel(store, guild, text).map(|i| i.to_string()),
                    kind::ROLE => search_query::resolve_role(store, guild, text).map(|i| i.to_string()),
                    kind::MENTIONABLE => search_query::resolve_user(store, guild, channel, text)
                        .or_else(|| search_query::resolve_role(store, guild, text))
                        .map(|i| i.to_string()),
                    _ => None,
                }
            };
            command_form::answers(&options, &entries, &resolve)
        };

        match result {
            Ok(answers) => {
                let command = form.command.clone();
                self.backend.send(Command::RunCommand { guild, channel, session_id, command, answers });
                self.command_form = None;
            }
            Err(message) => {
                if let Some(form) = &mut self.command_form {
                    form.error = Some(message);
                }
            }
        }
        cx.notify();
    }

    /// The form above the composer: the command, one row per option, Run.
    pub(super) fn render_command_form(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.command_form.as_ref()?;
        let mut card = div()
            .mb_2()
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::BG_PANEL))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .child(format!("/{}", form.command.name)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(rgb(theme::TEXT_MUTED))
                            .child(form.command.description.clone()),
                    ),
            );

        for (index, field) in form.fields.iter().enumerate() {
            let label = format!("{}{}", field.option.name, if field.option.required { " *" } else { "" });
            let control: AnyElement = if let Some(input) = &field.input {
                Input::new(input).into_any_element()
            } else if field.option.kind == kind::BOOLEAN {
                let on = matches!(field.entry, Entry::Bool(true));
                let mut track = div()
                    .id(("cmd-bool", index as u64))
                    .w(px(44.))
                    .h(px(24.))
                    .p(px(3.))
                    .flex()
                    .rounded_full()
                    .cursor_pointer()
                    .bg(rgb(if on { theme::ONLINE } else { theme::BG_ACTIVE }));
                // The knob sits at the end when the switch is on.
                if on {
                    track = track.justify_end();
                }
                track
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(field) = this.command_form.as_mut().and_then(|f| f.fields.get_mut(index)) {
                            field.entry = Entry::Bool(!matches!(field.entry, Entry::Bool(true)));
                        }
                        cx.notify();
                    }))
                    .child(div().size(px(18.)).rounded_full().bg(rgb(theme::TEXT_BRIGHT)))
                    .into_any_element()
            } else {
                let mut chips = div().flex().flex_wrap().gap_1();
                for (choice_index, choice) in field.option.choices.iter().enumerate() {
                    let selected = field.entry == Entry::Choice(choice_index);
                    chips = chips.child(
                        div()
                            .id(SharedString::from(format!("cmd-choice-{index}-{choice_index}")))
                            .h(px(26.))
                            .px_2()
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .text_size(px(13.))
                            .text_color(rgb(theme::TEXT_BRIGHT))
                            .bg(rgb(if selected { theme::ACCENT } else { theme::BG_ACTIVE }))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(field) =
                                    this.command_form.as_mut().and_then(|f| f.fields.get_mut(index))
                                {
                                    field.entry = Entry::Choice(choice_index);
                                }
                                cx.notify();
                            }))
                            .child(choice.name.clone()),
                    );
                }
                chips.into_any_element()
            };
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_size(px(12.)).text_color(rgb(theme::TEXT_MUTED)).child(label))
                    .child(control),
            );
        }

        if let Some(error) = &form.error {
            card = card.child(div().text_color(rgb(theme::DANGER)).child(error.clone()));
        }
        card = card.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .id("cmd-run")
                        .h(px(30.))
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_1()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .bg(rgb(theme::ACCENT))
                        .text_color(rgb(theme::TEXT_BRIGHT))
                        .hover(|s| s.opacity(0.85))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run_command(cx)))
                        .child(icon("send", 14., theme::TEXT_BRIGHT))
                        .child("Run"),
                )
                .child(
                    div()
                        .id("cmd-cancel")
                        .h(px(30.))
                        .px_3()
                        .flex()
                        .items_center()
                        .rounded(px(6.))
                        .cursor_pointer()
                        .bg(rgb(theme::BG_ACTIVE))
                        .text_color(rgb(theme::TEXT))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.command_form = None;
                            cx.notify();
                        }))
                        .child("Cancel"),
                ),
        );
        Some(card.into_any_element())
    }
}
