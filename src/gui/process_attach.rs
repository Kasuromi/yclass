use crate::{process::ProcessEntry, state::StateRef};
use eframe::{
    egui::{Context, RichText, ScrollArea, TextEdit, Window},
    epaint::{vec2, FontId},
};

pub struct ProcessAttachWindow {
    shown: bool,
    filter: String,
    processes: Vec<ProcessEntry>,
    state: StateRef,
}

impl ProcessAttachWindow {
    pub fn new(state: StateRef) -> Self {
        Self {
            state,
            processes: vec![],
            shown: false,
            filter: "".to_owned(),
        }
    }

    pub fn toggle(&mut self) {
        self.shown = !self.shown;

        if self.shown {
            self.update_processes();
        }
    }

    pub fn show(&mut self, ctx: &Context) -> Option<u32> {
        if !self.shown {
            return None;
        }

        // I promise I won't use it later :/
        let shown = unsafe { &mut (*(self as *mut Self)).shown };
        let mut attach_pid = None;

        Window::new("Attach to process")
            .collapsible(false)
            .open(shown)
            .default_size(vec2(180., 320.))
            .show(ctx, |ui| {
                ui.vertical_centered_justified(|ui| {
                    let r = TextEdit::singleline(&mut self.filter)
                        .desired_width(f32::INFINITY)
                        .hint_text("Filter by name")
                        .show(ui)
                        .response;

                    if ui.button("Refresh").clicked() || r.changed() {
                        self.update_processes();
                    }

                    ui.add_space(4.);
                    ui.separator();
                    ui.add_space(4.);

                    ScrollArea::vertical().show(ui, |ui| {
                        for pe in self.processes.iter().filter(|pe| {
                            self.filter.is_empty()
                                || pe.name.to_lowercase().contains(&self.filter.to_lowercase())
                        }) {
                            if ui
                                .button(
                                    RichText::new(format!("{} - {}", pe.name, pe.id))
                                        .font(FontId::proportional(16.)),
                                )
                                .clicked()
                            {
                                attach_pid = Some(pe.id);
                            }
                        }
                    });
                });
            });

        attach_pid
    }

    fn update_processes(&mut self) {
        let mut state = self.state.borrow_mut();
        let out = state.process.read().all_processes();

        match out {
            Ok(p) => self.processes = p,
            Err(e) => {
                _ = state
                    .toasts
                    .error(format!("Failed to iterate over processes. {e}"))
            }
        }
    }
}
