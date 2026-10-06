// SPDX-FileCopyrightText: 2026 Nikolay Govorov
// SPDX-License-Identifier: MPL-2.0

use crate::ui::classes;
use maud::{Markup, Render, html};

pub enum HeadingLevel {
    Two,
    Three,
}

pub struct SectionHeading<'a> {
    pub level: HeadingLevel,
    pub id: Option<&'a str>,
    pub label: &'a str,
}

impl Render for SectionHeading<'_> {
    fn render(&self) -> Markup {
        match self.level {
            HeadingLevel::Two => html! {
                h2 class=(classes::section::HEADING) { (self.label) }
            },
            HeadingLevel::Three => {
                html! {
                    h3 class=(classes::section::HEADING) id=[self.id] {
                        @if let Some(id) = self.id {
                            a href=(format!("#{id}")) { (self.label) }
                        } @else {
                            (self.label)
                        }
                    }
                }
            }
        }
    }
}
